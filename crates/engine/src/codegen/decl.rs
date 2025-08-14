use super::{Codegen, MUTEX_STRIDE};

pub fn decl_externals(g: &mut Codegen) {
    g.line("declare i32 @putchar(i32)");
    g.line("declare i32 @getchar()");
    g.line("declare i32 @fflush(i8*)");
    g.line("declare i32 @nanosleep(%timespec*, %timespec*)");
    g.line("declare i8* @malloc(i64)");
    g.line("declare void @free(i8*)");
    g.line("declare i32 @pthread_create(i64*, i8*, i8* (i8*)*, i8*)");
    g.line("declare i32 @pthread_join(i64, i8**)");
    g.line("declare i32 @pthread_mutex_init(i8*, i8*)");
    g.line("declare i32 @pthread_mutex_lock(i8*)");
    g.line("declare i32 @pthread_mutex_unlock(i8*)");
    g.line("declare i32 @pthread_cond_init(i8*, i8*)");
    g.line("declare i32 @pthread_cond_wait(i8*, i8*)");
    g.line("declare i32 @pthread_cond_broadcast(i8*)");

    if g.sanitize {
        g.line("declare i64 @pthread_self()");

        // Thread sanitizer functions
        g.line("declare void @tsan_read(%State*)");
        g.line("declare void @tsan_write(%State*)");
        g.line("declare void @tsan_fork(i64)");
        g.line("declare void @tsan_join(i64)");
    }

    // memcpy intrinsic (used for expanding the lock stack)
    g.line("declare void @llvm.memcpy.p0i8.p0i8.i64(i8* nocapture writeonly, i8* nocapture readonly, i64, i1 immarg)");
}

pub fn define_runtime_helpers(g: &mut Codegen) {
    // GEP: pointer to current cell
    g.line("define internal i8* @bf_gep_cell_ptr(%State* nocapture nonnull %S) alwaysinline nounwind {");
    g.indent += 1;
    g.line("%fld_base = getelementptr %State, %State* %S, i32 0, i32 0");
    g.line("%base = load i8*, i8** %fld_base");
    g.line("%fld_idx = getelementptr %State, %State* %S, i32 0, i32 1");
    g.line("%idx  = load i64,  i64*  %fld_idx");
    g.line("%p    = getelementptr i8, i8* %base, i64 %idx");
    g.line("ret i8* %p");
    g.indent -= 1;
    g.line("}");

    // Address of lock slot (mutex_slab + idx * stride)
    g.line("define internal i8* @bf_lock_slot_addr(%State* nocapture nonnull %S, i64 %idx) alwaysinline nounwind {");
    g.indent += 1;
    g.line("%fld_slab = getelementptr %State, %State* %S, i32 0, i32 2");
    g.line("%slab = load i8*, i8** %fld_slab");
    g.line(&format!("%off  = mul i64 %idx, {MUTEX_STRIDE}"));
    g.line("%slot = getelementptr i8, i8* %slab, i64 %off");
    g.line("ret i8* %slot");
    g.indent -= 1;
    g.line("}");

    // push_lock(%S, idx) with dynamic growth
    g.line("define internal void @push_lock(%State* nocapture nonnull %S, i64 %idx) nounwind {");
    g.indent += 1;
    g.line("%fld_sp = getelementptr %State, %State* %S, i32 0, i32 4");
    g.line("%sp  = load i64,  i64*  %fld_sp");
    g.line("%fld_cap = getelementptr %State, %State* %S, i32 0, i32 5");
    g.line("%cap = load i64,  i64*  %fld_cap");
    // Precompute fld_buf before branch for dominance
    g.line("%fld_buf = getelementptr %State, %State* %S, i32 0, i32 3");
    g.line("%need_grow = icmp eq i64 %sp, %cap");
    g.line("br i1 %need_grow, label %grow, label %push");

    g.line("grow:");
    g.line("%oldbuf = load i64*, i64** %fld_buf");
    g.line("%oldcap = load i64, i64* %fld_cap");
    g.line("%newcap = shl i64 %oldcap, 1");
    g.line("%oldbytes = mul i64 %oldcap, 8");
    g.line("%newbytes = mul i64 %newcap, 8");
    g.line("%newraw = call i8* @malloc(i64 %newbytes)");
    g.line("%newbuf = bitcast i8* %newraw to i64*");
    g.line("%dst = bitcast i64* %newbuf to i8*");
    g.line("%src = bitcast i64* %oldbuf to i8*");
    g.line("call void @llvm.memcpy.p0i8.p0i8.i64(i8* %dst, i8* %src, i64 %oldbytes, i1 false)");
    g.line("call void @free(i8* %src)");
    g.line("store i64* %newbuf, i64** %fld_buf");
    g.line("store i64  %newcap, i64*  %fld_cap");
    g.line("br label %push");

    g.line("push:");
    g.line("%buf = load i64*, i64** %fld_buf");
    g.line("%slotp = getelementptr i64, i64* %buf, i64 %sp");
    g.line("store i64 %idx, i64* %slotp");
    g.line("%sp1 = add i64 %sp, 1");
    g.line("store i64 %sp1, i64* %fld_sp");
    g.line("ret void");
    g.indent -= 1;
    g.line("}");

    // pop_lock(%S) -> i64 (caller assumes non-empty stack)
    g.line("define internal i64 @pop_lock(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.line("%fld_sp2 = getelementptr %State, %State* %S, i32 0, i32 4");
    g.line("%sp  = load i64, i64* %fld_sp2");
    g.line("%sp1 = add i64 %sp, -1");
    g.line("store i64 %sp1, i64* %fld_sp2");
    g.line("%fld_buf2 = getelementptr %State, %State* %S, i32 0, i32 3");
    g.line("%buf = load i64*, i64** %fld_buf2");
    g.line("%slotp = getelementptr i64, i64* %buf, i64 %sp1");
    g.line("%idx = load i64, i64* %slotp");
    g.line("ret i64 %idx");
    g.indent -= 1;
    g.line("}");

    // Primitive: pointer movement
    g.line("define internal void @bf_inc_ptr(%State* nocapture nonnull %S, i64 %delta) alwaysinline nounwind {");
    g.indent += 1;
    g.line("%fld_ptr = getelementptr %State,%State* %S, i32 0, i32 1");
    g.line("%v = load i64, i64* %fld_ptr");
    g.line("%u = add i64 %v, %delta");
    g.line("store i64 %u, i64* %fld_ptr");
    g.line("ret void");
    g.indent -= 1;
    g.line("}");

    // Primitive: add to cell (non-atomic)
    g.line("define internal void @bf_add_cell(%State* nocapture nonnull %S, i32 %delta) alwaysinline nounwind {");
    g.indent += 1;
    g.line("%p = call i8* @bf_gep_cell_ptr(%State* %S)");
    if g.sanitize {
        g.line("call void @tsan_read(%State* %S)");
    }
    g.line("%v0 = load i8, i8* %p");
    g.line("%d8 = trunc i32 %delta to i8");
    g.line("%v1 = add i8 %v0, %d8");
    if g.sanitize {
        g.line("call void @tsan_write(%State* %S)");
    }
    g.line("store i8 %v1, i8* %p");
    g.line("ret void");
    g.indent -= 1;
    g.line("}");

    // Output
    g.line("define internal void @bf_output(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.line("%p = call i8* @bf_gep_cell_ptr(%State* %S)");
    if g.sanitize {
        g.line("call void @tsan_read(%State* %S)");
    }
    g.line("%v = load i8, i8* %p");
    g.line("%w = zext i8 %v to i32");
    g.line("call i32 @putchar(i32 %w)");
    g.line("call i32 @fflush(i8* null)");
    g.line("ret void");
    g.indent -= 1;
    g.line("}");

    // Input
    g.line("define internal void @bf_input(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.line("%c = call i32 @getchar()");
    g.line("%eof = icmp slt i32 %c, 0");
    g.line("%cz = select i1 %eof, i32 0, i32 %c");
    g.line("%b = trunc i32 %cz to i8");
    g.line("%p = call i8* @bf_gep_cell_ptr(%State* %S)");
    if g.sanitize {
        g.line("call void @tsan_write(%State* %S)");
    }
    g.line("store i8 %b, i8* %p");
    g.line("ret void");
    g.indent -= 1;
    g.line("}");

    // Sleep (0.1s * ticks)
    g.line("define internal void @bf_sleep(i32 %ticks) nounwind {");
    g.indent += 1;
    g.line("%t64 = zext i32 %ticks to i64");
    g.line("%ns_total = mul i64 %t64, 100000000");
    g.line("%sec  = udiv i64 %ns_total, 1000000000");
    g.line("%nsec = urem i64 %ns_total, 1000000000");
    g.line("%ts = alloca %timespec");
    g.line("%ts_sec  = getelementptr %timespec, %timespec* %ts, i32 0, i32 0");
    g.line("%ts_nsec = getelementptr %timespec, %timespec* %ts, i32 0, i32 1");
    g.line("store i64 %sec,  i64* %ts_sec");
    g.line("store i64 %nsec, i64* %ts_nsec");
    g.line("call i32 @nanosleep(%timespec* %ts, %timespec* null)");
    g.line("ret void");
    g.indent -= 1;
    g.line("}");

    // Acquire lock (then push)
    g.line("define internal void @bf_lock_acquire(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.line("%fld_ptr2 = getelementptr %State,%State* %S, i32 0, i32 1");
    g.line("%idx = load i64, i64* %fld_ptr2");
    g.line("%slot = call i8* @bf_lock_slot_addr(%State* %S, i64 %idx)");
    g.line("call i32 @pthread_mutex_lock(i8* %slot)");
    g.line("call void @push_lock(%State* %S, i64 %idx)");
    g.line("ret void");
    g.indent -= 1;
    g.line("}");

    // Release lock (pop then unlock)
    g.line("define internal void @bf_lock_release(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.line("%idx = call i64 @pop_lock(%State* %S)");
    g.line("%slot = call i8* @bf_lock_slot_addr(%State* %S, i64 %idx)");
    g.line("call i32 @pthread_mutex_unlock(i8* %slot)");
    g.line("ret void");
    g.indent -= 1;
    g.line("}");

    // Address of condvar slot: cond_slab + idx * stride
    g.line("define internal i8* @bf_cond_slot_addr(%State* nocapture nonnull %S, i64 %idx) alwaysinline nounwind {");
    g.indent += 1;
    g.line("%csl = load i8*, i8** @cond_slab");
    g.line(&format!("%coff = mul i64 %idx, {MUTEX_STRIDE}"));
    g.line("%cslot = getelementptr i8, i8* %csl, i64 %coff");
    g.line("ret i8* %cslot");
    g.indent -= 1;
    g.line("}");

    // Address of cond-mutex slot: cond_mtx_slab + idx * stride
    g.line("define internal i8* @bf_cmtx_slot_addr(%State* nocapture nonnull %S, i64 %idx) alwaysinline nounwind {");
    g.indent += 1;
    g.line("%msl = load i8*, i8** @cond_mtx_slab");
    g.line(&format!("%moff = mul i64 %idx, {MUTEX_STRIDE}"));
    g.line("%mslot = getelementptr i8, i8* %msl, i64 %moff");
    g.line("ret i8* %mslot");
    g.indent -= 1;
    g.line("}");

    // Wait: lock cond-mutex -> cond_wait -> unlock
    g.line("define internal void @bf_wait(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.line("%fld_ptrW = getelementptr %State, %State* %S, i32 0, i32 1");
    g.line("%idxW = load i64, i64* %fld_ptrW");
    g.line("%cmW = call i8* @bf_cmtx_slot_addr(%State* %S, i64 %idxW)");
    g.line("%cvW = call i8* @bf_cond_slot_addr(%State* %S, i64 %idxW)");
    g.line("call i32 @pthread_mutex_lock(i8* %cmW)");
    g.line("call i32 @pthread_cond_wait(i8* %cvW, i8* %cmW)");
    g.line("call i32 @pthread_mutex_unlock(i8* %cmW)");
    g.line("ret void");
    g.indent -= 1;
    g.line("}");

    // Notify-all: lock cond-mutex -> broadcast -> unlock
    g.line("define internal void @bf_notify(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.line("%fld_ptrN = getelementptr %State, %State* %S, i32 0, i32 1");
    g.line("%idxN = load i64, i64* %fld_ptrN");
    g.line("%cmN = call i8* @bf_cmtx_slot_addr(%State* %S, i64 %idxN)");
    g.line("%cvN = call i8* @bf_cond_slot_addr(%State* %S, i64 %idxN)");
    g.line("call i32 @pthread_mutex_lock(i8* %cmN)");
    g.line("call i32 @pthread_cond_broadcast(i8* %cvN)");
    g.line("call i32 @pthread_mutex_unlock(i8* %cmN)");
    g.line("ret void");
    g.indent -= 1;
    g.line("}");
}
