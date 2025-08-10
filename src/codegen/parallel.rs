use super::{Codegen, LOCK_STACK_INIT, emit};

use crate::parser::Node;

/// 並列ブロック `{ A | B | ... }`
/// 親 %S を参照しつつ、各枝に独立 State を用意して起動・join。
pub fn emit_parallel(g: &mut Codegen, parent_s: &str, branches: &[Vec<Node>]) {
    let pid = g.uniq;
    g.uniq += 1;
    let k = branches.len();

    // 1) 各枝の thunk / thread_start を前方定義
    for (i, b) in branches.iter().enumerate() {
        let tname = format!("p{pid}_{i}");
        g.define_thunk(&tname, b);
        g.wln(&format!(
            "define internal i8* @thread_start_{tname}(i8* %arg) nounwind {{"
        ));
        g.indent += 1;
        g.wln("%S = bitcast i8* %arg to %State*");
        g.wln(&format!("call void @thunk_{tname}(%State* %S)"));
        g.wln("ret i8* null");
        g.indent -= 1;
        g.wln("}");
        g.wln("");
    }

    // 2) 親関数本体で threads 配列を確保して起動
    g.wln(&format!("%threads{pid} = alloca [{} x i64]", k));
    for i in 0..k {
        let child = fresh(g, "Schild");
        // sizeof(%State)
        g.wln(&format!("%st_bytes{pid}_{i} = ptrtoint (%State* getelementptr(%State, %State* null, i32 1) to i64)"));
        g.wln(&format!(
            "%st{pid}_{i} = call i8* @malloc(i64 %st_bytes{pid}_{i})"
        ));
        g.wln(&format!("{child} = bitcast i8* %st{pid}_{i} to %State*"));

        // tape_base / ptr_index / mutex_slab の継承
        // base
        g.wln(&format!("%fld_parent_base{pid}_{i} = getelementptr %State,%State* {parent_s}, i32 0, i32 0"));
        g.wln(&format!("%base{pid}_{i} = load i8*, i8** %fld_parent_base{pid}_{i}"));
        g.wln(&format!("%fld_child_base{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 0"));
        g.wln(&format!("store i8* %base{pid}_{i}, i8** %fld_child_base{pid}_{i}"));
        // idx
        g.wln(&format!("%fld_parent_idx{pid}_{i} = getelementptr %State,%State* {parent_s}, i32 0, i32 1"));
        g.wln(&format!("%idx{pid}_{i}  = load i64,  i64*  %fld_parent_idx{pid}_{i}"));
        g.wln(&format!("%fld_child_idx{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 1"));
        g.wln(&format!("store i64 %idx{pid}_{i},  i64*  %fld_child_idx{pid}_{i}"));
        // slab
        g.wln(&format!("%fld_parent_sl{pid}_{i} = getelementptr %State,%State* {parent_s}, i32 0, i32 2"));
        g.wln(&format!("%sl{pid}_{i}   = load i8*, i8** %fld_parent_sl{pid}_{i}"));
        g.wln(&format!("%fld_child_sl{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 2"));
        g.wln(&format!("store i8* %sl{pid}_{i},   i8** %fld_child_sl{pid}_{i}"));

        // lock stack 新規
        g.wln(&format!("%lsz{pid}_{i} = mul i64 {LOCK_STACK_INIT}, 8"));
        g.wln(&format!(
            "%stk{pid}_{i} = call i8* @malloc(i64 %lsz{pid}_{i})"
        ));
        g.wln(&format!(
            "%stk64{pid}_{i} = bitcast i8* %stk{pid}_{i} to i64*"
        ));
        g.wln(&format!("%fld_child_stk{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 3"));
        g.wln(&format!("store i64* %stk64{pid}_{i}, i64** %fld_child_stk{pid}_{i}"));
        g.wln(&format!("%fld_child_sp{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 4"));
        g.wln(&format!("store i64 0, i64* %fld_child_sp{pid}_{i}"));
        g.wln(&format!("%fld_child_cap{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 5"));
        g.wln(&format!("store i64 {LOCK_STACK_INIT}, i64* %fld_child_cap{pid}_{i}"));

        // pthread_create
        g.wln(&format!(
            "%tptr{pid}_{i} = getelementptr [{k} x i64], [{k} x i64]* %threads{pid}, i64 0, i64 {i}"
        ));
        g.wln(&format!("%arg{pid}_{i} = bitcast %State* {child} to i8*"));
        g.wln(&format!(
            "call i32 @pthread_create(i64* %tptr{pid}_{i}, i8* null, i8* (i8*)* @thread_start_p{pid}_{i}, i8* %arg{pid}_{i})"
        ));
    }

    // 3) join
    for i in 0..k {
        g.wln(&format!("%tval{pid}_{i} = getelementptr [{k} x i64], [{k} x i64]* %threads{pid}, i64 0, i64 {i}"));
        g.wln(&format!("%tload{pid}_{i} = load i64, i64* %tval{pid}_{i}"));
        g.wln(&format!(
            "call i32 @pthread_join(i64 %tload{pid}_{i}, i8** null)"
        ));
        // （必要なら %Schild の解放、stack の解放を追記できる）
    }
}

fn fresh(g: &mut Codegen, p: &str) -> String {
    g.fresh(p)
}
