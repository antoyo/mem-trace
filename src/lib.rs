//! Create another lib which will work with a demon:
//! this way, even if the process fork, all the data will be sent to the same process.

// TODO: also support posix_memalign
// TODO: add features to be able to only disable the tracer.
// TODO: count allocated memory (- freed memory) for different GCC allocators.
// TODO: track where allocated memory is from.

use core::mem;
#[cfg(feature="profiler")]
use std::hash::Hash;
#[cfg(feature="profiler")]
use std::time::{Duration, Instant};
use std::process;
#[cfg(feature="profiler")]
use std::thread;
#[cfg(feature="profiler")]
use std::thread::JoinHandle;

use backtrace::Backtrace;
use dashmap::{DashMap, DashSet};
use libc::{self, RTLD_NEXT, c_int, c_void, dlsym, size_t};
// TODO: switch to std::sync::LazyLock.
use once_cell::sync::Lazy;

#[cfg(feature="profiler")]
const ALLOCATION_COUNT_TO_PRINT: usize = 10;
#[cfg(feature="profiler")]
const PRINT_INTERVAL: Duration = Duration::from_secs(30);

static MAP: Lazy<DashMap<usize, Backtrace>> = Lazy::new(|| {
    DashMap::new()
});

#[cfg(feature="profiler")]
static USAGE_MAP: Lazy<DashMap<BacktraceWrapper, usize>> = Lazy::new(|| {
    DashMap::new()
});

static SET: Lazy<DashSet<u32>> = Lazy::new(|| {
    DashSet::new()
});

#[cfg(feature="profiler")]
static THREAD: Lazy<JoinHandle<()>> = Lazy::new(|| {
    thread::spawn(move || {
        let mut time = Instant::now();
        loop {
            let duration = time.elapsed();
            if duration > PRINT_INTERVAL {
                time = Instant::now();

                println!("Allocations:");

                let mut usage: Vec<_> = USAGE_MAP.iter()
                    .map(|elem| (elem.key().clone(), *elem.value()))
                    .collect();
                usage.sort_unstable_by(|(_, size1), (_, size2)| size2.cmp(size1));

                for (index, (backtrace, size)) in usage.iter_mut().take(ALLOCATION_COUNT_TO_PRINT).enumerate().rev() {
                    backtrace.0.resolve();
                    println!("# {}: {} bytes\n{:?}", index, size, backtrace.0);
                }
            }
        }
    })
});

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(feature="profiler")]
#[derive(Clone)]
struct BacktraceWrapper(Backtrace);

#[cfg(feature="profiler")]
impl PartialEq for BacktraceWrapper {
    fn eq(&self, other: &Self) -> bool {
        let self_frames = self.0.frames();
        let other_frames = other.0.frames();

        if self_frames.len() != other_frames.len() {
            return false;
        }

        for (self_frame, other_frame) in self_frames.iter().zip(other_frames) {
            if self_frame.ip() != other_frame.ip() {
                return false;
            }
        }

        true
    }
}

#[cfg(feature="profiler")]
impl Eq for BacktraceWrapper {
}

#[cfg(feature="profiler")]
impl Hash for BacktraceWrapper {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let ips: Vec<_> = self.0.frames()
            .iter()
            .map(|frame| frame.ip())
            .collect();
        ips.hash(state);
    }
}

#[cfg(feature="profiler")]
fn use_thread() {
    Lazy::force(&THREAD);
}

macro_rules! allocator_function {
    ($cstring:expr, $name:ident ( $size_param:ident: $size_type:ty $(,$param:ident: $type:ty)* )) => {
#[unsafe(no_mangle)]
pub extern "C" fn $name($size_param: $size_type $(,$param: $type)*) -> *mut c_void {
    #[cfg(feature="profiler")]
    use_thread();

    let pid = process::id();
    if !SET.contains(&pid) {
        println!("***** New PID for {}: {}", stringify!($name), pid);
        SET.insert(pid);
    }

    let backtrace = Backtrace::new_unresolved();

    #[cfg(feature="profiler")]
    {
        let mut usage = USAGE_MAP.entry(BacktraceWrapper(backtrace.clone()))
            .or_default();
        *usage += $size_param as usize;
    }

    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, $cstring.as_ptr());
        let orig_malloc: fn($size_type, $($type),*) -> *mut c_void = mem::transmute(orig_malloc);
        let ptr = orig_malloc($size_param, $($param),*);
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
