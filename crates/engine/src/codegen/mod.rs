use std::fmt::Write as _;

use crate::parser::Node;

mod decl;
mod emit;
mod parallel;

pub const TAPE_LEN: i64 = 30_000;
pub const MUTEX_STRIDE: i64 = 64;
pub const LOCK_STACK_INIT: i64 = 16;

pub fn generate_ir(nodes: &[Node], sanitize: bool) -> String {
    let mut cg = Codegen::new(sanitize);
    cg.preamble(); // globals, %State, declarations, runtime helper definitions
    cg.defer_thunk("main", nodes); // Defer creation of thunk for main
    cg.define_main(); // Initialize @main then call @thunk_main
    cg.flush_deferred(); // Emit all deferred function definitions at the end
    cg.finish()
}

pub struct Codegen {
    out: String,
    indent: usize,
    pub uniq: usize,
    deferred: Vec<String>, // Function definitions deferred for later emission
    pub sanitize: bool,    // Whether to generate code with sanitization checks
}

impl Codegen {
    fn new(sanitize: bool) -> Self {
        Self {
            out: String::with_capacity(32 * 1024),
            indent: 0,
            uniq: 0,
            deferred: Vec::new(),
            sanitize,
        }
    }

    pub fn finish(self) -> String {
        self.out
    }

    fn line(&mut self, s: &str) {
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
        // Temporarily swap output buffer & indent to build a deferred definition
        let mut saved_out = String::new();
        std::mem::swap(&mut self.out, &mut saved_out);
        let saved_indent = self.indent;
        self.indent = 0;
        f(self);
        let produced = std::mem::take(&mut self.out);
        // Restore previous buffer & indent
        self.out = saved_out;
        self.indent = saved_indent;
        produced
    }

    /// Create a thunk from an arbitrary sequence of nodes and defer its emission
    pub fn defer_thunk(&mut self, name: &str, nodes: &[Node]) {
        let ir = self.with_temp_buffer(|this| {
            this.line(&format!(
                "define internal void @thunk_{name}(%State* nocapture nonnull %S) {{"
            ));
            this.indent += 1;
            this.label("entry");
            emit::emit_nodes(this, "%S", nodes);
            this.line("ret void");
            this.indent -= 1;
            this.line("}");
            this.line("");
        });
        self.push_def(ir);
    }

    /// Defer generation of thread_start_* wrapper function
    pub fn defer_thread_start(&mut self, tname: &str) {
        let ir = self.with_temp_buffer(|this| {
            this.line(&format!(
                "define internal i8* @thread_start_{tname}(i8* %arg) nounwind {{"
            ));
            this.indent += 1;
            this.line("%S = bitcast i8* %arg to %State*");
            if this.sanitize {
                // Post parent thread ID to TSAN
                this.line("%fld_tid = getelementptr %State, %State* %S, i32 0, i32 6");
                this.line("%tid_parent = load i64, i64* %fld_tid");
                this.line("call void @tsan_fork(i64 %tid_parent)");

                // Initialize thread ID if sanitization is enabled
                this.line("%tid_self = call i64 @pthread_self()");
                this.line("store i64 %tid_self, i64* %fld_tid");
            }
            this.line(&format!("call void @thunk_{tname}(%State* %S)"));
            this.line("ret i8* null");
            this.indent -= 1;
            this.line("}");
            this.line("");
        });
        self.push_def(ir);
    }

    fn preamble(&mut self) {
        // Shared tape and mutex slot slab (memory allocated at program start)
        self.line(&format!(
            "@tape = internal global [{TAPE_LEN} x i8] zeroinitializer"
        ));
        self.line("@mutex_slab = internal global i8* null");
        self.line("@cond_slab = internal global i8* null");
        self.line("@cond_mtx_slab = internal global i8* null");
        if self.sanitize {
            self.line(
                "%State = type { i8*, i64, i8*, i64*, i64, i64, i64 } ; (tape, ptr, slab, stack, sp, cap, tid)",
            );
        } else {
            self.line(
                "%State = type { i8*, i64, i8*, i64*, i64, i64 } ; (tape, ptr, slab, stack, sp, cap)",
            );
        }
        self.line("%timespec = type { i64, i64 } ; (tv_sec, tv_nsec)");
        self.line("");
        decl::decl_externals(self);
        decl::define_runtime_helpers(self);
        self.line("");
    }

    fn define_main(&mut self) {
        self.line("define i32 @main() {");
        self.indent += 1;
        self.label("entry");
        // Allocate & initialize mutex_slab
        self.line(&format!("%slab_bytes = mul i64 {TAPE_LEN}, {MUTEX_STRIDE}"));
        self.line("%slab = call i8* @malloc(i64 %slab_bytes)");
        self.line("store i8* %slab, i8** @mutex_slab");
        // Allocate & initialize cond_slab and cond_mtx_slab
        self.line(&format!("%cond_bytes = mul i64 {TAPE_LEN}, {MUTEX_STRIDE}"));
        self.line("%cslab = call i8* @malloc(i64 %cond_bytes)");
        self.line("store i8* %cslab, i8** @cond_slab");
        self.line("%cmslab = call i8* @malloc(i64 %cond_bytes)");
        self.line("store i8* %cmslab, i8** @cond_mtx_slab");
        // pthread_mutex_init for every cell
        self.line("%i = alloca i64");
        self.line("store i64 0, i64* %i");
        self.line("br label %init.loop");
        self.label("init.loop");
        self.line("%cur = load i64, i64* %i");
        self.line(&format!("%cond = icmp slt i64 %cur, {TAPE_LEN}"));
        self.line("br i1 %cond, label %init.body, label %init.end");
        self.label("init.body");
        self.line(&format!("%off = mul i64 %cur, {MUTEX_STRIDE}"));
        self.line("%sl0 = load i8*, i8** @mutex_slab");
        self.line("%slot = getelementptr i8, i8* %sl0, i64 %off");
        self.line("call i32 @pthread_mutex_init(i8* %slot, i8* null)");
        self.line("%cl0 = load i8*, i8** @cond_slab");
        self.line("%cslot = getelementptr i8, i8* %cl0, i64 %off");
        self.line("call i32 @pthread_cond_init(i8* %cslot, i8* null)");
        self.line("%ml0 = load i8*, i8** @cond_mtx_slab");
        self.line("%mslot2 = getelementptr i8, i8* %ml0, i64 %off");
        self.line("call i32 @pthread_mutex_init(i8* %mslot2, i8* null)");
        self.line("%cur1 = add i64 %cur, 1");
        self.line("store i64 %cur1, i64* %i");
        self.line("br label %init.loop");
        self.label("init.end");
        // Allocate & initialize initial State
        self.line("%st_end = getelementptr %State, %State* null, i32 1");
        self.line("%st_bytes = ptrtoint %State* %st_end to i64");
        self.line("%st = call i8* @malloc(i64 %st_bytes)");
        self.line("%S = bitcast i8* %st to %State*");
        self.line(&format!(
            "%base = getelementptr [{TAPE_LEN} x i8], [{TAPE_LEN} x i8]* @tape, i64 0, i64 0"
        ));
        let f0 = self.fresh("fld");
        self.line(&format!(
            "{f0} = getelementptr %State, %State* %S, i32 0, i32 0"
        ));
        self.line(&format!("store i8* %base, i8** {f0}"));
        let f1 = self.fresh("fld");
        self.line(&format!(
            "{f1} = getelementptr %State, %State* %S, i32 0, i32 1"
        ));
        self.line(&format!("store i64 0, i64* {f1}"));
        self.line("%sl = load i8*, i8** @mutex_slab");
        let f2 = self.fresh("fld");
        self.line(&format!(
            "{f2} = getelementptr %State, %State* %S, i32 0, i32 2"
        ));
        self.line(&format!("store i8* %sl, i8** {f2}"));
        self.line(&format!("%lsz = mul i64 {LOCK_STACK_INIT}, 8"));
        self.line("%stk = call i8* @malloc(i64 %lsz)");
        self.line("%stk64 = bitcast i8* %stk to i64*");
        let f3 = self.fresh("fld");
        self.line(&format!(
            "{f3} = getelementptr %State, %State* %S, i32 0, i32 3"
        ));
        self.line(&format!("store i64* %stk64, i64** {f3}"));
        let f4 = self.fresh("fld");
        self.line(&format!(
            "{f4} = getelementptr %State, %State* %S, i32 0, i32 4"
        ));
        self.line(&format!("store i64 0, i64* {f4}"));
        let f5 = self.fresh("fld");
        self.line(&format!(
            "{f5} = getelementptr %State, %State* %S, i32 0, i32 5"
        ));
        self.line(&format!("store i64 {LOCK_STACK_INIT}, i64* {f5}"));
        if self.sanitize {
            // Initialize thread ID if sanitization is enabled
            self.line("%tid = call i64 @pthread_self()");
            let f6 = self.fresh("fld");
            self.line(&format!(
                "{f6} = getelementptr %State, %State* %S, i32 0, i32 6"
            ));
            self.line(&format!("store i64 %tid, i64* {f6}"));
        }
        // Run top-level program
        self.line("call void @thunk_main(%State* %S)");
        self.line("ret i32 0");
        self.indent -= 1;
        self.line("}");
    }
}
