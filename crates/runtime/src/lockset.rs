use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use crate::{Cell, State, Tid};

struct Access {
    tid: Tid,
    is_write: bool,
    lockset: HashSet<Cell>,
}

#[derive(Default)]
struct CellHist {
    last: Option<Access>,
}

static HIST: LazyLock<Mutex<HashMap<Cell, CellHist>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn current_lockset(s: &State) -> HashSet<Cell> {
    let sp = s.lock_sp;
    if sp <= 0 || s.lock_stack.is_null() {
        return HashSet::new();
    }
    let src: &[Cell] = unsafe { core::slice::from_raw_parts(s.lock_stack, sp as usize) };
    src.iter().copied().collect()
}

pub unsafe fn lockset_check(s: *const State, is_write: bool) {
    let s = unsafe { s.as_ref().expect("State pointer is null") };

    let idx = s.ptr_index;
    let cur_locks = current_lockset(s);
    if cur_locks.contains(&idx) {
        return;
    }

    let tid = unsafe { libc::pthread_self() } as usize as Tid;
    let mut map = HIST.lock().unwrap();
    let entry = map.entry(idx).or_default();

    if let Some(prev) = &entry.last
        && prev.tid != tid
        && prev.lockset.is_disjoint(&cur_locks)
        && (is_write || prev.is_write)
    {
        eprintln!(
            "[TSAN] race({}) cell={} prev{{tid:{}, write:{}}} now{{tid:{}, write:{}}}",
            if is_write { "write" } else { "read" },
            idx,
            prev.tid,
            prev.is_write,
            tid,
            is_write
        );
    }

    entry.last = Some(Access {
        tid,
        is_write,
        lockset: cur_locks,
    });
}
