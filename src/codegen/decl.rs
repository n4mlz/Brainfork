use super::{Codegen, LOCK_STACK_INIT, MUTEX_STRIDE, TAPE_LEN};

pub fn decl_externals(g: &mut Codegen) {
    g.wln("declare i32 @putchar(i32)");
    g.wln("declare i32 @getchar()");
    g.wln("declare i32 @usleep(i32)");
    g.wln("declare i8* @malloc(i64)");
    g.wln("declare void @free(i8*)");
    g.wln("declare i32 @pthread_create(i64*, i8*, i8* (i8*)*, i8*)");
    g.wln("declare i32 @pthread_join(i64, i8**)");
    g.wln("declare i32 @pthread_mutex_init(i8*, i8*)");
    g.wln("declare i32 @pthread_mutex_lock(i8*)");
    g.wln("declare i32 @pthread_mutex_unlock(i8*)");
    // memcpy intrinsic（stack の拡張に使用）
    g.wln("declare void @llvm.memcpy.p0i8.p0i8.i64(i8* nocapture writeonly, i8* nocapture readonly, i64, i1 immarg)");
}

pub fn define_runtime_helpers(g: &mut Codegen) {
    // --- GEP: 現在セルのポインタ ---
    g.wln("define internal i8* @bf_gep_cell_ptr(%State* nocapture nonnull %S) alwaysinline nounwind {");
    g.indent += 1;
    g.wln("%fld_base = getelementptr %State, %State* %S, i32 0, i32 0");
    g.wln("%base = load i8*, i8** %fld_base");
    g.wln("%fld_idx = getelementptr %State, %State* %S, i32 0, i32 1");
    g.wln("%idx  = load i64,  i64*  %fld_idx");
    g.wln("%p    = getelementptr i8, i8* %base, i64 %idx");
    g.wln("ret i8* %p");
    g.indent -= 1;
    g.wln("}");

    // --- ロックスロットのアドレス（mutex_slab + idx*stride） ---
    g.wln("define internal i8* @bf_lock_slot_addr(%State* nocapture nonnull %S, i64 %idx) alwaysinline nounwind {");
    g.indent += 1;
    g.wln("%fld_slab = getelementptr %State, %State* %S, i32 0, i32 2");
    g.wln("%slab = load i8*, i8** %fld_slab");
    g.wln(&format!("%off  = mul i64 %idx, {MUTEX_STRIDE}"));
    g.wln("%slot = getelementptr i8, i8* %slab, i64 %off");
    g.wln("ret i8* %slot");
    g.indent -= 1;
    g.wln("}");

    // --- push_lock(%S, idx) : 動的拡張あり ---
    g.wln("define internal void @push_lock(%State* nocapture nonnull %S, i64 %idx) nounwind {");
    g.indent += 1;
    g.wln("%fld_sp = getelementptr %State, %State* %S, i32 0, i32 4");
    g.wln("%sp  = load i64,  i64*  %fld_sp");
    g.wln("%fld_cap = getelementptr %State, %State* %S, i32 0, i32 5");
    g.wln("%cap = load i64,  i64*  %fld_cap");
    g.wln("%need_grow = icmp eq i64 %sp, %cap");
    g.wln("br i1 %need_grow, label %grow, label %push");

    g.wln("grow:");
    g.wln("%fld_buf = getelementptr %State, %State* %S, i32 0, i32 3");
    g.wln("%oldbuf = load i64*, i64** %fld_buf");
    g.wln("%oldcap = load i64, i64* %fld_cap");
    g.wln("%newcap = shl i64 %oldcap, 1");
    g.wln("%oldbytes = mul i64 %oldcap, 8");
    g.wln("%newbytes = mul i64 %newcap, 8");
    g.wln("%newraw = call i8* @malloc(i64 %newbytes)");
    g.wln("%newbuf = bitcast i8* %newraw to i64*");
    g.wln("%dst = bitcast i64* %newbuf to i8*");
    g.wln("%src = bitcast i64* %oldbuf to i8*");
    g.wln("call void @llvm.memcpy.p0i8.p0i8.i64(i8* %dst, i8* %src, i64 %oldbytes, i1 false)");
    g.wln("call void @free(i8* %src)");
    g.wln("store i64* %newbuf, i64** %fld_buf");
    g.wln("store i64  %newcap, i64*  %fld_cap");
    g.wln("br label %push");

    g.wln("push:");
    g.wln("%buf = load i64*, i64** %fld_buf");
    g.wln("%slotp = getelementptr i64, i64* %buf, i64 %sp");
    g.wln("store i64 %idx, i64* %slotp");
    g.wln("%sp1 = add i64 %sp, 1");
    g.wln("store i64 %sp1, i64* %fld_sp");
    g.wln("ret void");
    g.indent -= 1;
    g.wln("}");

    // --- pop_lock(%S) -> i64（空は未定義：パーサ側で検査前提） ---
    g.wln("define internal i64 @pop_lock(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.wln("%fld_sp2 = getelementptr %State, %State* %S, i32 0, i32 4");
    g.wln("%sp  = load i64, i64* %fld_sp2");
    g.wln("%sp1 = add i64 %sp, -1");
    g.wln("store i64 %sp1, i64* %fld_sp2");
    g.wln("%fld_buf2 = getelementptr %State, %State* %S, i32 0, i32 3");
    g.wln("%buf = load i64*, i64** %fld_buf2");
    g.wln("%slotp = getelementptr i64, i64* %buf, i64 %sp1");
    g.wln("%idx = load i64, i64* %slotp");
    g.wln("ret i64 %idx");
    g.indent -= 1;
    g.wln("}");

    // --- 基本命令: ポインタ移動 ---
    g.wln("define internal void @bf_inc_ptr(%State* nocapture nonnull %S, i64 %delta) alwaysinline nounwind {");
    g.indent += 1;
    g.wln("%fld_ptr = getelementptr %State,%State* %S, i32 0, i32 1");
    g.wln("%v = load i64, i64* %fld_ptr");
    g.wln("%u = add i64 %v, %delta");
    g.wln("store i64 %u, i64* %fld_ptr");
    g.wln("ret void");
    g.indent -= 1;
    g.wln("}");

    // --- 基本命令: セル加算（非 atomic） ---
    g.wln("define internal void @bf_add_cell(%State* nocapture nonnull %S, i32 %delta) alwaysinline nounwind {");
    g.indent += 1;
    g.wln("%p = call i8* @bf_gep_cell_ptr(%State* %S)");
    g.wln("%v0 = load i8, i8* %p");
    g.wln("%d8 = trunc i32 %delta to i8");
    g.wln("%v1 = add i8 %v0, %d8");
    g.wln("store i8 %v1, i8* %p");
    g.wln("ret void");
    g.indent -= 1;
    g.wln("}");

    // --- 出力 ---
    g.wln("define internal void @bf_output(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.wln("%p = call i8* @bf_gep_cell_ptr(%State* %S)");
    g.wln("%v = load i8, i8* %p");
    g.wln("%w = zext i8 %v to i32");
    g.wln("call i32 @putchar(i32 %w)");
    g.wln("ret void");
    g.indent -= 1;
    g.wln("}");

    // --- 入力 ---
    g.wln("define internal void @bf_input(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.wln("%c = call i32 @getchar()");
    g.wln("%eof = icmp slt i32 %c, 0");
    g.wln("%cz = select i1 %eof, i32 0, i32 %c");
    g.wln("%b = trunc i32 %cz to i8");
    g.wln("%p = call i8* @bf_gep_cell_ptr(%State* %S)");
    g.wln("store i8 %b, i8* %p");
    g.wln("ret void");
    g.indent -= 1;
    g.wln("}");

    // --- 待機（0.1s * ticks）---
    g.wln("define internal void @bf_wait(i32 %ticks) nounwind {");
    g.indent += 1;
    g.wln("%u = mul i32 %ticks, 100000");
    g.wln("call i32 @usleep(i32 %u)");
    g.wln("ret void");
    g.indent -= 1;
    g.wln("}");

    // --- ロック取得（成功後に push） ---
    g.wln("define internal void @bf_lock_acquire(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.wln("%fld_ptr2 = getelementptr %State,%State* %S, i32 0, i32 1");
    g.wln("%idx = load i64, i64* %fld_ptr2");
    g.wln("%slot = call i8* @bf_lock_slot_addr(%State* %S, i64 %idx)");
    g.wln("call i32 @pthread_mutex_lock(i8* %slot)");
    g.wln("call void @push_lock(%State* %S, i64 %idx)");
    g.wln("ret void");
    g.indent -= 1;
    g.wln("}");

    // --- ロック解放（pop した idx に対して unlock） ---
    g.wln("define internal void @bf_lock_release(%State* nocapture nonnull %S) nounwind {");
    g.indent += 1;
    g.wln("%idx = call i64 @pop_lock(%State* %S)");
    g.wln("%slot = call i8* @bf_lock_slot_addr(%State* %S, i64 %idx)");
    g.wln("call i32 @pthread_mutex_unlock(i8* %slot)");
    g.wln("ret void");
    g.indent -= 1;
    g.wln("}");
}
