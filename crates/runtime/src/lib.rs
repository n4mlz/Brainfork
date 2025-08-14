#![allow(clippy::missing_safety_doc)]

mod lockset;
mod vector_clock;

type Tid = u64;
type Cell = i64;

const TAPE_LEN: usize = 30000;

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

#[derive(Debug, Clone, Copy)]
pub struct Race {
    pub cell: Cell,
    pub is_write: bool,
}

impl core::fmt::Display for Race {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "race({}) cell={}",
            if self.is_write { "write" } else { "read" },
            self.cell
        )
    }
}

fn merge_race(r1: Race, r2: Race) -> Race {
    Race {
        cell: r1.cell,
        is_write: r1.is_write || r2.is_write,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_write(s: *const State) {
    let res1 = unsafe { lockset::lockset_check(s, true) };
    let res2 = vector_clock::vector_clock_write(s);

    if let (Err(r1), Err(r2)) = (res1, res2) {
        let combined = merge_race(r1, r2);
        eprintln!("[TSan] {combined}");
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_read(s: *const State) {
    let res1 = unsafe { lockset::lockset_check(s, false) };
    let res2 = vector_clock::vector_clock_read(s);

    if let (Err(r1), Err(r2)) = (res1, res2) {
        let combined = merge_race(r1, r2);
        eprintln!("[TSan] {combined}");
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_acquire(s: *const State, idx: Cell) {
    vector_clock::vector_clock_acquire(s, idx);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_release(s: *const State, idx: Cell) {
    vector_clock::vector_clock_release(s, idx);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_fork(parent_tid: Tid) {
    let child_tid = unsafe { libc::pthread_self() } as usize as Tid;
    vector_clock::vector_clock_fork(parent_tid, child_tid);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_join(child_tid: Tid) {
    let parent_tid = unsafe { libc::pthread_self() } as usize as Tid;
    vector_clock::vector_clock_join(parent_tid, child_tid);
}
