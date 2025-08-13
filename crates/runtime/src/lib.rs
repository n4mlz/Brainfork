#![allow(clippy::missing_safety_doc)]

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

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

static HIST: LazyLock<Mutex<HashMap<i64, CellHist>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

unsafe fn tsan_access(s: *const State, is_write: bool) {
    let s = unsafe { s.as_ref().expect("State pointer is null") };

    let idx = s.ptr_index;
    let cur_locks = current_lockset(s);
    if cur_locks.contains(&idx) {
        return;
    }

    let tid = unsafe { libc::pthread_self() } as usize as u64;
    let mut map = HIST.lock().unwrap();
    let entry = map.entry(idx).or_default();
    let tree = THREAD_TREE.lock().unwrap();

    if let Some(prev) = &entry.last
        && prev.tid != tid
        && !tree.is_ancestor(prev.tid, tid)
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_write(s: *const State) {
    unsafe { tsan_access(s, true) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_read(s: *const State) {
    unsafe { tsan_access(s, false) };
}

struct ThreadTree {
    parent: HashMap<u64, u64>,
}

impl ThreadTree {
    fn new() -> Self {
        Self {
            parent: HashMap::new(),
        }
    }

    fn add_edge(&mut self, parent_id: u64, child_id: u64) {
        self.parent.insert(child_id, parent_id);
    }

    fn is_descendant(&self, prev_id: u64, current_id: u64) -> bool {
        let mut cur = current_id;
        while let Some(&p) = self.parent.get(&cur) {
            if p == prev_id {
                return true;
            }
            cur = p;
        }
        false
    }

    fn is_ancestor(&self, id1: u64, id2: u64) -> bool {
        self.is_descendant(id1, id2) || self.is_descendant(id2, id1)
    }
}

static THREAD_TREE: LazyLock<Mutex<ThreadTree>> = LazyLock::new(|| Mutex::new(ThreadTree::new()));

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsan_post_parent_tid(parent_tid: u64) {
    let child_tid = unsafe { libc::pthread_self() } as usize as u64;

    let mut tree = THREAD_TREE.lock().unwrap();
    tree.add_edge(parent_tid, child_tid);
}
