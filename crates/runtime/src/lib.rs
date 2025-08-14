#![allow(clippy::missing_safety_doc)]

mod lockset;
mod vector_clock;

type Tid = u64;
type Cell = i64;

const TAPE_LEN: usize = 30000;

#[derive(Debug, Clone)]
pub enum TsanError {
    Race { cell: Cell, is_write: bool },
}

impl core::fmt::Display for TsanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TsanError::Race { cell, is_write } => write!(
                f,
                "[TSAN] race({}) cell={cell}",
                if *is_write { "write" } else { "read" }
            ),
        }
    }
}

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

fn report_if_both_race(lockset_res: Result<(), TsanError>, vc_res: Result<(), TsanError>) {
    if let (
        Err(TsanError::Race {
            cell: c1,
            is_write: w1,
        }),
        Err(TsanError::Race {
            cell: c2,
            is_write: w2,
        }),
    ) = (lockset_res, vc_res)
        && c1 == c2
    {
        eprintln!(
            "{}",
            TsanError::Race {
                cell: c1,
                is_write: w1 || w2,
            }
        );
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_write(s: *const State) {
    report_if_both_race(
        unsafe { lockset::lockset_check(s, true) },
        vector_clock::vector_clock_write(s),
    );
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_read(s: *const State) {
    report_if_both_race(
        unsafe { lockset::lockset_check(s, false) },
        vector_clock::vector_clock_read(s),
    );
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
