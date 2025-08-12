#![allow(clippy::missing_safety_doc)]

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

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

fn current_lockset(s: &State) -> HashSet<i64> {
    let sp = s.lock_sp;
    if sp <= 0 || s.lock_stack.is_null() {
        return HashSet::new();
    }
    let src: &[i64] = unsafe { core::slice::from_raw_parts(s.lock_stack, sp as usize) };
    src.iter().copied().collect()
}

struct Access {
    tid: u64,
    is_write: bool,
    lockset: HashSet<i64>,
}

#[derive(Default)]
struct CellHist {
    last: Option<Access>,
}

static HIST: OnceLock<Mutex<HashMap<i64, CellHist>>> = OnceLock::new();
fn hist() -> &'static Mutex<HashMap<i64, CellHist>> {
    HIST.get_or_init(|| Mutex::new(HashMap::new()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_write(s: *const State) {
    let s = unsafe { s.as_ref().expect("State pointer is null") };

    let idx = s.ptr_index;
    let cur_locks = current_lockset(s);
    if cur_locks.contains(&idx) {
        return;
    }

    let tid = unsafe { libc::pthread_self() as usize as u64 };
    let mut map = hist().lock().unwrap();
    let entry = map.entry(idx).or_default();

    if let Some(prev) = &entry.last
        && prev.tid != tid
        && prev.lockset.is_disjoint(&cur_locks)
    {
        eprintln!(
            "[TSAN] race(write) cell={} prev{{tid:{}, write:{}}} now{{tid:{}, write:true}}",
            idx, prev.tid, prev.is_write, tid
        );
    }

    entry.last = Some(Access {
        tid,
        is_write: true,
        lockset: cur_locks,
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_read(s: *const State) {
    let s = unsafe { s.as_ref().expect("State pointer is null") };

    let idx = s.ptr_index;
    let cur_locks = current_lockset(s);
    if cur_locks.contains(&idx) {
        return;
    }

    let tid = unsafe { libc::pthread_self() as usize as u64 };
    let mut map = hist().lock().unwrap();
    let entry = map.entry(idx).or_default();

    if let Some(prev) = &entry.last
        && prev.is_write
        && prev.tid != tid
        && prev.lockset.is_disjoint(&cur_locks)
    {
        eprintln!(
            "[TSAN] race(read)  cell={} prev{{tid:{}, write:true}} now{{tid:{}, write:false}}",
            idx, prev.tid, tid
        );
    }

    entry.last = Some(Access {
        tid,
        is_write: false,
        lockset: cur_locks,
    });
}
