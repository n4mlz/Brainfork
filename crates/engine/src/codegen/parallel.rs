use super::{Codegen, LOCK_STACK_INIT};

use crate::parser::Node;

/// Prepare independent State for each branch while sharing parent %S, then start and join threads
pub fn emit_parallel(g: &mut Codegen, parent_s: &str, branches: &[Vec<Node>]) {
    let pid = g.uniq;
    g.uniq += 1;
    let k = branches.len();

    // Defer thunk / thread_start for each branch
    for (i, b) in branches.iter().enumerate() {
        let tname = format!("p{pid}_{i}");
        g.defer_thunk(&tname, b);
        g.defer_thread_start(&tname);
    }

    // In parent function: allocate threads array and launch
    g.line(&format!("%threads{pid} = alloca [{k} x i64]"));
    for i in 0..k {
        let child = fresh(g, "Schild");
        // Separate GEP for struct size calculation
        g.line(&format!(
            "%st_end{pid}_{i} = getelementptr %State, %State* null, i32 1"
        ));
        g.line(&format!(
            "%st_bytes{pid}_{i} = ptrtoint %State* %st_end{pid}_{i} to i64"
        ));
        g.line(&format!(
            "%st{pid}_{i} = call i8* @malloc(i64 %st_bytes{pid}_{i})"
        ));
        g.line(&format!("{child} = bitcast i8* %st{pid}_{i} to %State*"));
        // base
        g.line(&format!(
            "%fld_parent_base{pid}_{i} = getelementptr %State,%State* {parent_s}, i32 0, i32 0"
        ));
        g.line(&format!(
            "%base{pid}_{i} = load i8*, i8** %fld_parent_base{pid}_{i}"
        ));
        g.line(&format!(
            "%fld_child_base{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 0"
        ));
        g.line(&format!(
            "store i8* %base{pid}_{i}, i8** %fld_child_base{pid}_{i}"
        ));
        // idx
        g.line(&format!(
            "%fld_parent_idx{pid}_{i} = getelementptr %State,%State* {parent_s}, i32 0, i32 1"
        ));
        g.line(&format!(
            "%idx{pid}_{i}  = load i64,  i64*  %fld_parent_idx{pid}_{i}"
        ));
        g.line(&format!(
            "%fld_child_idx{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 1"
        ));
        g.line(&format!(
            "store i64 %idx{pid}_{i},  i64*  %fld_child_idx{pid}_{i}"
        ));
        // slab
        g.line(&format!(
            "%fld_parent_sl{pid}_{i} = getelementptr %State,%State* {parent_s}, i32 0, i32 2"
        ));
        g.line(&format!(
            "%sl{pid}_{i}   = load i8*, i8** %fld_parent_sl{pid}_{i}"
        ));
        g.line(&format!(
            "%fld_child_sl{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 2"
        ));
        g.line(&format!(
            "store i8* %sl{pid}_{i},   i8** %fld_child_sl{pid}_{i}"
        ));
        // lock stack
        g.line(&format!("%lsz{pid}_{i} = mul i64 {LOCK_STACK_INIT}, 8"));
        g.line(&format!(
            "%stk{pid}_{i} = call i8* @malloc(i64 %lsz{pid}_{i})"
        ));
        g.line(&format!(
            "%stk64{pid}_{i} = bitcast i8* %stk{pid}_{i} to i64*"
        ));
        g.line(&format!(
            "%fld_child_stk{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 3"
        ));
        g.line(&format!(
            "store i64* %stk64{pid}_{i}, i64** %fld_child_stk{pid}_{i}"
        ));
        g.line(&format!(
            "%fld_child_sp{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 4"
        ));
        g.line(&format!("store i64 0, i64* %fld_child_sp{pid}_{i}"));
        g.line(&format!(
            "%fld_child_cap{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 5"
        ));
        g.line(&format!(
            "store i64 {LOCK_STACK_INIT}, i64* %fld_child_cap{pid}_{i}"
        ));
        if g.sanitize {
            // thread ID
            g.line(&format!(
                "%fld_parent_tid{pid}_{i} = getelementptr %State,%State* {parent_s}, i32 0, i32 6"
            ));
            g.line(&format!(
                "%tid{pid}_{i} = load i64, i64* %fld_parent_tid{pid}_{i}"
            ));
            g.line(&format!(
                "%fld_child_tid{pid}_{i} = getelementptr %State,%State* {child}, i32 0, i32 6"
            ));
            g.line(&format!(
                "store i64 %tid{pid}_{i}, i64* %fld_child_tid{pid}_{i}"
            ));
        }
        // pthread_create
        g.line(&format!(
            "%tptr{pid}_{i} = getelementptr [{k} x i64], [{k} x i64]* %threads{pid}, i64 0, i64 {i}"
        ));
        g.line(&format!("%arg{pid}_{i} = bitcast %State* {child} to i8*"));
        g.line(&format!("call i32 @pthread_create(i64* %tptr{pid}_{i}, i8* null, i8* (i8*)* @thread_start_p{pid}_{i}, i8* %arg{pid}_{i})"));
    }

    // Join all threads
    for i in 0..k {
        g.line(&format!(
            "%tval{pid}_{i} = getelementptr [{k} x i64], [{k} x i64]* %threads{pid}, i64 0, i64 {i}"
        ));
        g.line(&format!("%tload{pid}_{i} = load i64, i64* %tval{pid}_{i}"));
        g.line(&format!(
            "call i32 @pthread_join(i64 %tload{pid}_{i}, i8** null)"
        ));
        if g.sanitize {
            g.line(&format!("call void @tsan_join(i64 %tload{pid}_{i})"));
        }
    }
}

fn fresh(g: &mut Codegen, p: &str) -> String {
    g.fresh(p)
}
