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
    c"malloc",
    malloc(size: size_t)
);

// Operator new.
allocator_function!(
    c"_Znwm",
    _Znwm(size: size_t)
);

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
