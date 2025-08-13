use std::io::{Read, Result, Write};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::parser::Node;

pub trait RBound = Read + Send + 'static;
pub trait WBound = Write + Send + 'static;

const MEMORY_SIZE: usize = 30000;

pub struct Interpreter<R: RBound, W: WBound> {
    input: Arc<Mutex<R>>,
    output: Arc<Mutex<W>>,
}

impl<R: RBound, W: WBound> Interpreter<R, W> {
    pub fn new(input: R, output: W) -> Self {
        Interpreter {
            input: Arc::new(Mutex::new(input)),
            output: Arc::new(Mutex::new(output)),
        }
    }

    pub fn run(&self, nodes: &[Node]) -> Result<()> {
        let memory = Arc::new((0..MEMORY_SIZE).map(|_| AtomicU8::new(0)).collect());
        let locks = Arc::new((0..MEMORY_SIZE).map(|_| AtomicBool::new(false)).collect());

        ThreadState::new(memory, locks, self.input.clone(), self.output.clone()).run(nodes);
        Ok(())
    }
}

struct ThreadState<R: RBound, W: WBound> {
    memory: Arc<Vec<AtomicU8>>,
    locks: Arc<Vec<AtomicBool>>,
    input: Arc<Mutex<R>>,
    output: Arc<Mutex<W>>,
    ptr: usize,
    lock_stack: Vec<usize>,
}

impl<R: RBound, W: WBound> ThreadState<R, W> {
    fn new(
        memory: Arc<Vec<AtomicU8>>,
        locks: Arc<Vec<AtomicBool>>,
        input: Arc<Mutex<R>>,
        output: Arc<Mutex<W>>,
    ) -> Self {
        ThreadState {
            memory,
            locks,
            input,
            output,
            ptr: 0,
            lock_stack: Vec::new(),
        }
    }

    fn run(&mut self, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::IncPtr => {
                    self.ptr = self.ptr.wrapping_add(1) % MEMORY_SIZE;
                }
                Node::DecPtr => {
                    self.ptr = self.ptr.wrapping_sub(1) % MEMORY_SIZE;
                }
                Node::IncCell => {
                    self.memory[self.ptr].fetch_add(1, Ordering::SeqCst);
                }
                Node::DecCell => {
                    self.memory[self.ptr].fetch_sub(1, Ordering::SeqCst);
                }
                Node::Output => {
                    let byte = self.memory[self.ptr].load(Ordering::SeqCst);
                    let mut out = self.output.lock().unwrap();
                    out.write_all(&[byte]).unwrap();
                    out.flush().unwrap();
                }
                Node::Input => {
                    let mut buf = [0];
                    let mut inp = self.input.lock().unwrap();
                    if inp.read_exact(&mut buf).is_ok() {
                        self.memory[self.ptr].store(buf[0], Ordering::SeqCst);
                    }
                }
                Node::Loop(body) => {
                    while self.memory[self.ptr].load(Ordering::SeqCst) != 0 {
                        self.run(body);
                    }
                }
                Node::Parallel(branches) => {
                    let mut handles = Vec::new();
                    for branch in branches {
                        let mem = self.memory.clone();
                        let locks = self.locks.clone();
                        let inp = self.input.clone();
                        let out = self.output.clone();
                        let branch_clone = branch.clone();
                        handles.push(thread::spawn(move || {
                            let mut child = ThreadState::new(mem, locks, inp, out);
                            child.run(&branch_clone);
                        }));
                    }
                    for h in handles {
                        h.join().unwrap();
                    }
                }
                Node::LockAcquire => {
                    while self.locks[self.ptr]
                        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                        .is_err()
                    {
                        thread::yield_now();
                    }
                    self.lock_stack.push(self.ptr);
                }
                Node::LockRelease => {
                    if let Some(idx) = self.lock_stack.pop() {
                        self.locks[idx].store(false, Ordering::SeqCst);
                    } else {
                        panic!("LockRelease without matching LockAcquire");
                    }
                }
                Node::Sleep(count) => {
                    let dur = Duration::from_millis(100 * (*count as u64));
                    thread::sleep(dur);
                }
            }
        }
    }
}
