/// Create another lib which will work with a demon:
/// this way, even if the process fork, all the data will be sent to the same process.

// TODO: also support posix_memalign

use core::mem;
use std::process;

use backtrace::Backtrace;
use dashmap::{DashMap, DashSet};
use libc::{self, RTLD_NEXT, c_int, c_void, dlsym, size_t};
use once_cell::sync::Lazy;

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

static MAP: Lazy<DashMap<usize, Backtrace>> = Lazy::new(|| {
    DashMap::new()
});

static SET: Lazy<DashSet<u32>> = Lazy::new(|| {
    DashSet::new()
});

#[no_mangle]
pub extern "C" fn malloc(size: size_t) -> *mut c_void {
    let pid = process::id();
    if !SET.contains(&pid) {
        println!("******* New PID: {}", pid);
        SET.insert(pid);
    }

    let backtrace = Backtrace::new_unresolved();
    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, b"malloc\0".as_ptr() as *const i8);
        let orig_malloc: fn(size_t) -> *mut c_void = mem::transmute(orig_malloc);
        let ptr = orig_malloc(size);
        MAP.insert(ptr as usize, backtrace);
        //println!("malloc 0x{:x}", ptr as usize);
        ptr
    }
}

/*#[no_mangle]
pub extern "C" fn gcc_jit_context_new_rvalue_from_int(ctxt: *mut c_void, numeric_type: *mut c_void, value: c_int) -> *mut c_void {
    println!("new rvalue from int");
    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, b"gcc_jit_context_new_rvalue_from_int\0".as_ptr() as *const i8);
        let orig_malloc: fn(*mut c_void, *mut c_void, c_int) -> *mut c_void = mem::transmute(orig_malloc);
        orig_malloc(ctxt, numeric_type, value)
    }
}*/

#[no_mangle]
pub extern "C" fn _Znwm(size: size_t) -> *mut c_void {
    let pid = process::id();
    if !SET.contains(&pid) {
        println!("***** New PID for operator new: {}", pid);
        SET.insert(pid);
    }

    let backtrace = Backtrace::new_unresolved();
    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, b"_Znwm\0".as_ptr() as *const i8);
        let orig_malloc: fn(size_t) -> *mut c_void = mem::transmute(orig_malloc);
        let ptr = orig_malloc(size);
        MAP.insert(ptr as usize, backtrace);
        //println!("new 0x{:x}", ptr as usize);
        ptr
    }
}

#[no_mangle]
pub extern "C" fn print_trace(size: size_t) {
    println!("PID: {}", std::process::id());
    println!("Len: {}", MAP.len());
    if let Some(mut backtrace) = MAP.get_mut(&size) {
        backtrace.resolve();
        println!("{:?}", *backtrace);
    }
    else {
        println!("Can't find address 0x{:x}", size);
    }
}
