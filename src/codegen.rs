use std::fmt::Write as _;

use crate::parser::Node;

mod decl;
mod emit;
mod parallel;

pub const TAPE_LEN: i64 = 30_000;
pub const MUTEX_STRIDE: i64 = 64; // pthread_mutex_t のサイズが不明なので余裕を確保
pub const LOCK_STACK_INIT: i64 = 16;

pub fn generate_ir(nodes: &[Node]) -> String {
    let mut cg = Codegen::new();
    cg.preamble(); // globals, %State, declare群, ランタイム関数定義
    cg.defer_thunk("main", nodes); // main 用 thunk を遅延定義
    cg.define_main(); // @main 初期化 → @thunk_main 呼び出し
    cg.flush_deferred(); // 末尾に全遅延関数を出力
    cg.finish()
}

pub struct Codegen {
    out: String,
    indent: usize,
    pub uniq: usize,
    deferred: Vec<String>, // 遅延する関数定義
}

impl Codegen {
    fn new() -> Self {
        let this = Self {
            out: String::with_capacity(32 * 1024),
            indent: 0,
            uniq: 0,
            deferred: Vec::new(),
        };
        this
    }

    pub fn finish(self) -> String {
        self.out
    }

    fn wln(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        let _ = writeln!(self.out, "{s}");
    }
    pub fn label(&mut self, name: &str) {
        let _ = writeln!(self.out, "{name}:");
    }
    pub fn fresh(&mut self, prefix: &str) -> String {
        let id = self.uniq;
        self.uniq += 1;
        format!("%{prefix}{id}")
    }

    fn push_def(&mut self, def: String) {
        self.deferred.push(def);
    }
    fn flush_deferred(&mut self) {
        for def in std::mem::take(&mut self.deferred) {
            self.out.push_str(&def);
        }
    }

    fn with_temp_buffer<F: FnOnce(&mut Self)>(&mut self, f: F) -> String {
        // swap out current buffer & indent
        let mut saved_out = String::new();
        std::mem::swap(&mut self.out, &mut saved_out);
        let saved_indent = self.indent;
        self.indent = 0;
        f(self);
        let produced = std::mem::take(&mut self.out);
        // restore
        self.out = saved_out;
        self.indent = saved_indent;
        produced
    }

    /// 任意のノード列から thunk を作成し遅延出力
    pub fn defer_thunk(&mut self, name: &str, nodes: &[Node]) {
        let ir = self.with_temp_buffer(|this| {
            this.wln(&format!(
                "define internal void @thunk_{name}(%State* nocapture nonnull %S) {{"
            ));
            this.indent += 1;
            this.label("entry");
            emit::emit_nodes(this, "%S", nodes);
            this.wln("ret void");
            this.indent -= 1;
            this.wln("}");
            this.wln("");
        });
        self.push_def(ir);
    }

    /// thread_start_* ラッパー関数を遅延生成
    pub fn defer_thread_start(&mut self, tname: &str) {
        let ir = self.with_temp_buffer(|this| {
            this.wln(&format!(
                "define internal i8* @thread_start_{tname}(i8* %arg) nounwind {{"
            ));
            this.indent += 1;
            this.wln("%S = bitcast i8* %arg to %State*");
            this.wln(&format!("call void @thunk_{tname}(%State* %S)"));
            this.wln("ret i8* null");
            this.indent -= 1;
            this.wln("}");
            this.wln("");
        });
        self.push_def(ir);
    }

    fn preamble(&mut self) {
        // 共有テープと mutex スロットのスラブ（実体アドレスは起動時に確保）
        self.wln(&format!(
            "@tape = internal global [{TAPE_LEN} x i8] zeroinitializer"
        ));
        self.wln("@mutex_slab = internal global i8* null");
        self.wln(
            "%State = type { i8*, i64, i8*, i64*, i64, i64 } ; (tape, ptr, slab, stack, sp, cap)",
        );
        self.wln("");
        decl::decl_externals(self);
        decl::define_runtime_helpers(self); // push/pop, bf_* 命令など
        self.wln("");
    }

    fn define_main(&mut self) {
        self.wln("define i32 @main() {");
        self.indent += 1;
        self.label("entry");
        // mutex_slab を確保 & 初期化
        self.wln(&format!("%slab_bytes = mul i64 {TAPE_LEN}, {MUTEX_STRIDE}"));
        self.wln("%slab = call i8* @malloc(i64 %slab_bytes)");
        self.wln("store i8* %slab, i8** @mutex_slab");
        // 全セル分の pthread_mutex_init
        self.wln("%i = alloca i64");
        self.wln("store i64 0, i64* %i");
        self.label("init.loop");
        self.wln(&format!("%cur = load i64, i64* %i"));
        self.wln(&format!("%cond = icmp slt i64 %cur, {TAPE_LEN}"));
        self.wln("br i1 %cond, label %init.body, label %init.end");
        self.label("init.body");
        self.wln("%sl0 = load i8*, i8** @mutex_slab");
        self.wln(&format!("%off = mul i64 %cur, {MUTEX_STRIDE}"));
        self.wln("%slot = getelementptr i8, i8* %sl0, i64 %off");
        self.wln("call i32 @pthread_mutex_init(i8* %slot, i8* null)");
        self.wln("%cur1 = add i64 %cur, 1");
        self.wln("store i64 %cur1, i64* %i");
        self.wln("br label %init.loop");
        self.label("init.end");
        // 初期 State を確保・初期化
        self.wln(
            "%st_bytes = ptrtoint (%State* getelementptr(%State, %State* null, i32 1) to i64)",
        ); // 定数式 GEP はこのまま
        self.wln("%st = call i8* @malloc(i64 %st_bytes)");
        self.wln("%S = bitcast i8* %st to %State*");
        self.wln(&format!(
            "%base = getelementptr [{TAPE_LEN} x i8], [{TAPE_LEN} x i8]* @tape, i64 0, i64 0"
        ));
        let f0 = self.fresh("fld");
        self.wln(&format!(
            "{f0} = getelementptr %State, %State* %S, i32 0, i32 0"
        ));
        self.wln(&format!("store i8* %base, i8** {f0}"));
        let f1 = self.fresh("fld");
        self.wln(&format!(
            "{f1} = getelementptr %State, %State* %S, i32 0, i32 1"
        ));
        self.wln(&format!("store i64 0, i64* {f1}"));
        self.wln("%sl = load i8*, i8** @mutex_slab");
        let f2 = self.fresh("fld");
        self.wln(&format!(
            "{f2} = getelementptr %State, %State* %S, i32 0, i32 2"
        ));
        self.wln(&format!("store i8* %sl, i8** {f2}"));
        self.wln(&format!("%lsz = mul i64 {LOCK_STACK_INIT}, 8"));
        self.wln("%stk = call i8* @malloc(i64 %lsz)");
        self.wln("%stk64 = bitcast i8* %stk to i64*");
        let f3 = self.fresh("fld");
        self.wln(&format!(
            "{f3} = getelementptr %State, %State* %S, i32 0, i32 3"
        ));
        self.wln(&format!("store i64* %stk64, i64** {f3}"));
        let f4 = self.fresh("fld");
        self.wln(&format!(
            "{f4} = getelementptr %State, %State* %S, i32 0, i32 4"
        ));
        self.wln(&format!("store i64 0, i64* {f4}"));
        let f5 = self.fresh("fld");
        self.wln(&format!(
            "{f5} = getelementptr %State, %State* %S, i32 0, i32 5"
        ));
        self.wln(&format!("store i64 {LOCK_STACK_INIT}, i64* {f5}"));
        // トップレベル実行
        self.wln("call void @thunk_main(%State* %S)");
        self.wln("ret i32 0");
        self.indent -= 1;
        self.wln("}");
    }
}
