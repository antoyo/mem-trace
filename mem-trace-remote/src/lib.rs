// TODO: also support posix_memalign

use core::mem;
use std::io::{Cursor, Seek, Write};
use std::io::SeekFrom::Start;
use std::{net::TcpStream, sync::mpsc::{Sender, channel}, thread::{self, JoinHandle}};

use backtrace::Backtrace;
use libc::{self, RTLD_NEXT, c_void, dlsym, size_t};
use once_cell::sync::Lazy;
use rmp_serialize::Encoder;
use rustc_serialize::Encodable;

mod trace;

use trace::{Trace, HEADER_SIZE};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

static mut SENDER: Option<Sender<Trace>> = None;

static THREAD: Lazy<JoinHandle<()>> = Lazy::new(|| {
    println!("Creating channel");
    let (sender, receiver) = channel();
    unsafe {
        SENDER = Some(sender);
    }
    thread::spawn(move || {
        let mut stream = TcpStream::connect("127.0.0.1:45423").expect("connect");
        println!("Connected to stream");
        while let Ok(mut trace) = receiver.recv() {
            trace.backtrace.resolve();
            // Reserve space to write the size.
            let buffer = vec![0; HEADER_SIZE];
            let mut cursor = Cursor::new(buffer);
            if cursor.seek(Start(HEADER_SIZE as u64)).is_ok() {
                println!("Sending address: 0x{:X}", trace.address);
                match trace.encode(&mut Encoder::new(&mut &mut cursor)) {
                    Ok(_) => {
                        let mut buffer = cursor.into_inner();
                        let size = buffer.len() - HEADER_SIZE;
                        println!("Sending of size {}", size);
                        write_u32(&mut buffer, size as u32);
                        if let Err(error) = stream.write_all(&buffer) {
                            eprintln!("Send error: {}", error);
                        }
                    },
                    Err(error) => eprintln!("Failed to serialize message. {}", error),
                }
            }
            else {
                eprintln!("Failed to seek buffer.");
            }
        }
    })
});

#[no_mangle]
pub extern "C" fn malloc(size: size_t) -> *mut c_void {
    use_thread();
    println!("Malloc");
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
pub extern "C" fn _Znwm(size: size_t) -> *mut c_void {
    let backtrace = Backtrace::new_unresolved();
    unsafe {
        let orig_malloc = dlsym(RTLD_NEXT, b"_Znwm\0".as_ptr() as *const i8);
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
    unsafe {
        match SENDER {
            Some(ref sender) => {
                if let Err(error) = sender.send(Trace {
                    address,
                    backtrace,
                }) {
                    eprintln!("Cannot send trace: {}", error);
                }
            }
            None => eprintln!("No sender"),
        }
    }
}
