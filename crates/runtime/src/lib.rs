#![allow(clippy::missing_safety_doc)]

mod lockset;

#[repr(C)]
#[derive(Debug)]
pub struct State {
    tape_base: *mut i8,
    ptr_index: i64,
    mutex_slab: *mut i8,
    lock_stack: *mut i64,
    lock_sp: i64,
    lock_cap: i64,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_write(s: *const State) {
    unsafe { lockset::tsan_access(s, true) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_read(s: *const State) {
    unsafe { lockset::tsan_access(s, false) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_fork(parent_tid: u64) {
    let child_tid = unsafe { libc::pthread_self() } as usize as u64;

    println!("[TSAN] fork: parent_tid={parent_tid}, child_tid={child_tid}");
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_join(child_tid: u64) {
    let parent_tid = unsafe { libc::pthread_self() } as usize as u64;

    println!("[TSAN] join: parent_tid={parent_tid}, child_tid={child_tid}");
}
