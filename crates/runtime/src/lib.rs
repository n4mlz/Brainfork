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
pub extern "C" fn hello() {
    println!("Hello, world!");
}

#[unsafe(no_mangle)]
pub extern "C" fn print_state(state: &State) {
    println!("State:{state:?}");
}
