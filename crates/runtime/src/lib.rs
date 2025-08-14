#![allow(clippy::missing_safety_doc)]

mod lockset;

type Tid = u64;
type Cell = i64;

#[repr(C)]
#[derive(Debug)]
pub struct State {
    tape_base: *mut i8,
    ptr_index: Cell,
    mutex_slab: *mut i8,
    lock_stack: *mut i64,
    lock_sp: i64,
    lock_cap: i64,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_write(s: *const State) {
    unsafe { lockset::lockset_check(s, true) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_read(s: *const State) {
    unsafe { lockset::lockset_check(s, false) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_fork(parent_tid: Tid) {
    let child_tid = unsafe { libc::pthread_self() } as usize as Tid;

    println!("[TSAN] fork: parent_tid={parent_tid}, child_tid={child_tid}");
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_join(child_tid: Tid) {
    let parent_tid = unsafe { libc::pthread_self() } as usize as Tid;

    println!("[TSAN] join: parent_tid={parent_tid}, child_tid={child_tid}");
}
