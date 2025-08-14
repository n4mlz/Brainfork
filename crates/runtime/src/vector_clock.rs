use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use crate::{Cell, State, TAPE_LEN, Tid, TsanError};

type VectorClock = HashMap<Tid, u64>;

fn tick(c: &mut VectorClock, t: Tid) {
    *c.entry(t).or_insert(0) += 1;
}

fn leq(a: &VectorClock, b: &VectorClock) -> bool {
    for (k, av) in a {
        if *av > *b.get(k).unwrap_or(&0) {
            return false;
        }
    }
    true
}

fn join_in(a: &mut VectorClock, b: &VectorClock) {
    for (k, bv) in b {
        let e = a.entry(*k).or_insert(0);
        if *e < *bv {
            *e = *bv;
        }
    }
}

#[derive(Default)]
pub struct RaceDetector {
    ct: HashMap<Tid, VectorClock>,
    rx: Vec<VectorClock>,
    wx: Vec<VectorClock>,
    lm: Vec<VectorClock>,
}

impl RaceDetector {
    pub fn new() -> Self {
        Self {
            ct: HashMap::new(),
            rx: vec![HashMap::new(); TAPE_LEN],
            wx: vec![HashMap::new(); TAPE_LEN],
            lm: vec![HashMap::new(); TAPE_LEN],
        }
    }

    fn ct_mut(&mut self, t: Tid) -> &mut VectorClock {
        self.ct.entry(t).or_default()
    }

    pub fn rel(&mut self, t: Tid, m: Cell) {
        let ct = self.ct_mut(t);
        tick(ct, t);
        self.lm[m as usize] = ct.clone();
    }

    pub fn acq(&mut self, t: Tid, m: Cell) {
        let lm = self.lm[m as usize].clone();
        let ct = self.ct_mut(t);
        join_in(ct, &lm);
    }

    pub fn rd(&mut self, t: Tid, x: Cell) -> Result<(), TsanError> {
        let ct_snapshot = self.ct_mut(t).clone();
        if !leq(&self.wx[x as usize], &ct_snapshot) {
            return Err(TsanError::Race {
                cell: x,
                is_write: true,
            });
        }
        let my_time = *ct_snapshot.get(&t).unwrap_or(&0);
        self.rx[x as usize].insert(t, my_time);
        Ok(())
    }

    pub fn wr(&mut self, t: Tid, x: Cell) -> Result<(), TsanError> {
        let ct_snapshot = self.ct_mut(t).clone();
        if !leq(&self.wx[x as usize], &ct_snapshot) {
            return Err(TsanError::Race {
                cell: x,
                is_write: true,
            });
        }
        if !leq(&self.rx[x as usize], &ct_snapshot) {
            return Err(TsanError::Race {
                cell: x,
                is_write: false,
            });
        }
        let my_time = *ct_snapshot.get(&t).unwrap_or(&0);
        self.wx[x as usize].insert(t, my_time);
        Ok(())
    }

    pub fn fork(&mut self, t: Tid, u: Tid) {
        {
            let ct_parent = self.ct_mut(t);
            tick(ct_parent, t);
        }
        let mut cu = self.ct.get(&t).cloned().unwrap_or_default();
        cu.remove(&u);
        self.ct.insert(u, cu);
        tick(self.ct_mut(u), u);
    }

    pub fn join(&mut self, t: Tid, u: Tid) {
        let cu = self.ct.get(&u).cloned().unwrap_or_default();
        let ct = self.ct_mut(t);
        join_in(ct, &cu);
        tick(ct, t);
    }
}

static VECTOR_CLOCK: LazyLock<Mutex<RaceDetector>> =
    LazyLock::new(|| Mutex::new(RaceDetector::new()));

pub fn vector_clock_write(s: *const State) -> Result<(), TsanError> {
    let mut vector_clock = VECTOR_CLOCK.lock().unwrap();
    let tid = unsafe { libc::pthread_self() } as usize as Tid;
    let s = unsafe { s.as_ref().expect("State pointer is null") };

    vector_clock.wr(tid, s.ptr_index)
}

pub fn vector_clock_read(s: *const State) -> Result<(), TsanError> {
    let mut vector_clock = VECTOR_CLOCK.lock().unwrap();
    let tid = unsafe { libc::pthread_self() } as usize as Tid;
    let s = unsafe { s.as_ref().expect("State pointer is null") };

    vector_clock.rd(tid, s.ptr_index)
}

pub fn vector_clock_acquire(_: *const State, m: Cell) {
    let mut vector_clock = VECTOR_CLOCK.lock().unwrap();
    let tid = unsafe { libc::pthread_self() } as usize as Tid;

    vector_clock.acq(tid, m);
}

pub fn vector_clock_release(_: *const State, m: Cell) {
    let mut vector_clock = VECTOR_CLOCK.lock().unwrap();
    let tid = unsafe { libc::pthread_self() } as usize as Tid;

    vector_clock.rel(tid, m);
}

pub fn vector_clock_fork(parent_tid: Tid, child_tid: Tid) {
    let mut vector_clock = VECTOR_CLOCK.lock().unwrap();

    vector_clock.fork(parent_tid, child_tid);
}

pub fn vector_clock_join(parent_tid: Tid, child_tid: Tid) {
    let mut vector_clock = VECTOR_CLOCK.lock().unwrap();

    vector_clock.join(parent_tid, child_tid);
}
