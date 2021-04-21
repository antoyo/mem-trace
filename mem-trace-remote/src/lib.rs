// TODO: remove unsafe code.
//
// NOTE: somehow, operator new() is calling malloc from jemalloc.c, which is not the case on a
// normal C gccjit example.
// Rustc replace the allocator of linked C code.
//
// See: compiler/rustc/src/main.rs
// Disable jemalloc from the compiler to see all allocations.

use core::mem;
use std::io::{Cursor, Seek, Write};
use std::io::SeekFrom::Start;
use std::{net::TcpStream, thread::{self, JoinHandle}};

use backtrace::Backtrace;
use crossbeam::queue::SegQueue;
use libc::{self, RTLD_NEXT, c_int, c_void, dlsym, size_t};
use once_cell::sync::Lazy;
use rmp_serialize::Encoder;
use rustc_serialize::Encodable;

mod trace;

use trace::{Trace, HEADER_SIZE};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

static QUEUE: Lazy<SegQueue<Trace>> = Lazy::new(|| {
    SegQueue::new()
});

static THREAD: Lazy<JoinHandle<()>> = Lazy::new(|| {
    thread::spawn(move || {
        let mut stream = TcpStream::connect("127.0.0.1:45423").expect("connect");
        println!("Connected to stream");
        loop {
            if let Some(mut trace) = QUEUE.pop() {
                trace.backtrace.resolve();
                // Reserve space to write the size.
                let buffer = vec![0; HEADER_SIZE];
                let mut cursor = Cursor::new(buffer);
                if cursor.seek(Start(HEADER_SIZE as u64)).is_ok() {
                    match trace.encode(&mut Encoder::new(&mut &mut cursor)) {
                        Ok(_) => {
                            let mut buffer = cursor.into_inner();
                            let size = buffer.len() - HEADER_SIZE;
                            write_u32(&mut buffer, size as u32);
                            match stream.write_all(&buffer) {
                                Ok(()) => (),
                                Err(error) => eprintln!("Send error: {}", error),
                            }
                        },
                        Err(error) => eprintln!("Failed to serialize message. {}", error),
                    }
                }
                else {
                    eprintln!("Failed to seek buffer.");
                }
            }
        }
    })
});

#[no_mangle]
pub extern "C" fn malloc(size: size_t) -> *mut c_void {
    use_thread();
    let backtrace = Backtrace::new_unresolved();
    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, b"malloc\0".as_ptr() as *const i8);
        let orig_malloc: fn(size_t) -> *mut c_void = mem::transmute(orig_malloc);
        let ptr = orig_malloc(size);
        send(ptr as usize, backtrace);
        ptr
    }
}

#[no_mangle]
pub extern "C" fn posix_memalign(memptr: *mut *mut c_void, alignment: size_t, size: size_t) -> c_int {
    use_thread();
    let backtrace = Backtrace::new_unresolved();
    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, b"posix_memalign\0".as_ptr() as *const i8);
        let orig_malloc: fn(*mut *mut c_void, size_t, size_t) -> c_int = mem::transmute(orig_malloc);
        let res = orig_malloc(memptr, alignment, size);
        if res == 0 {
            send(*memptr as usize, backtrace);
        }
        res
    }
}

// TODO: remove this since it calls malloc anyway?
#[no_mangle]
pub extern "C" fn _Znwm(size: size_t) -> *mut c_void {
    use_thread();
    let backtrace = Backtrace::new_unresolved();
    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, b"_Znwm\0".as_ptr() as *const i8);
        let orig_malloc: fn(size_t) -> *mut c_void = mem::transmute(orig_malloc);
        let ptr = orig_malloc(size);
        send(ptr as usize, backtrace);
        ptr
    }
}

#[no_mangle]
pub extern "C" fn _Znam(size: size_t) -> *mut c_void {
    println!("new[]");
    use_thread();
    let backtrace = Backtrace::new_unresolved();
    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, b"_Znam\0".as_ptr() as *const i8);
        let orig_malloc: fn(size_t) -> *mut c_void = mem::transmute(orig_malloc);
        let ptr = orig_malloc(size);
        send(ptr as usize, backtrace);
        ptr
    }
}

fn use_thread() {
    Lazy::force(&THREAD);
}

fn write_u32(buffer: &mut [u8], size: u32) {
    if buffer.len() >= 4 {
        buffer[0] = (size & 0xFF) as u8;
        buffer[1] = ((size >> 8) & 0xFF) as u8;
        buffer[2] = ((size >> 16) & 0xFF) as u8;
        buffer[3] = ((size >> 24) & 0xFF) as u8;
    }
}

fn send(address: usize, backtrace: Backtrace) {
    QUEUE.push(Trace {
        address,
        backtrace,
    });
}
