//! Create another lib which will work with a demon:
//! this way, even if the process fork, all the data will be sent to the same process.

// TODO: also support posix_memalign
// TODO: count allocated memory (- freed memory) for different GCC allocators.
// TODO: track where allocated memory is from.

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

#[unsafe(no_mangle)]
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

/*#[unsafe(no_mangle)]
pub extern "C" fn gcc_jit_context_new_rvalue_from_int(ctxt: *mut c_void, numeric_type: *mut c_void, value: c_int) -> *mut c_void {
    println!("new rvalue from int");
    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, b"gcc_jit_context_new_rvalue_from_int\0".as_ptr() as *const i8);
        let orig_malloc: fn(*mut c_void, *mut c_void, c_int) -> *mut c_void = mem::transmute(orig_malloc);
        orig_malloc(ctxt, numeric_type, value)
    }
}*/

/// Operator new.
#[unsafe(no_mangle)]
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

macro_rules! allocator_function {
    ($cstring:expr, $name:ident ( $($param:ident: $type:ty),* )) => {
#[unsafe(no_mangle)]
pub extern "C" fn $name($($param: $type),*) -> *mut c_void {
    let pid = process::id();
    if !SET.contains(&pid) {
        println!("***** New PID for {}: {}", stringify!($name), pid);
        SET.insert(pid);
    }

    let backtrace = Backtrace::new_unresolved();
    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, $cstring.as_ptr());
        let orig_malloc: fn($($type),*) -> *mut c_void = mem::transmute(orig_malloc);
        let ptr = orig_malloc($($param),*);
        MAP.insert(ptr as usize, backtrace);
        ptr
    }
}
    };
}

allocator_function!(
    c"_Z26ggc_internal_cleared_allocmPFvPvEmm",
    _Z26ggc_internal_cleared_allocmPFvPvEmm(size: size_t, param1: *mut c_void, param2: size_t, param3: size_t)
);

allocator_function!(
    c"_Z9rtx_alloc8rtx_code",
    _Z9rtx_alloc8rtx_code(code: c_int)
);

#[unsafe(no_mangle)]
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
