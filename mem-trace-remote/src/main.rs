use std::io::{BufRead, BufReader, Read, Write, stdin, stdout};
use std::{net::{TcpListener, TcpStream}, thread};

use backtrace::Backtrace;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use rmp_serialize::Decoder;
use rustc_serialize::Decodable;

mod trace;

use trace::{Trace, HEADER_SIZE};

static MAP: Lazy<DashMap<usize, Backtrace>> = Lazy::new(|| {
    DashMap::new()
});

fn main() {
    thread::spawn(|| {
        let listener = TcpListener::bind("127.0.0.1:45423").expect("bind");
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_client(stream),
                Err(error) => eprintln!("Accept error: {}", error),
            }
        }
    });

    let mut stdout = stdout();
    print!("> ");
    let _ = stdout.flush();
    let stdin = stdin();
    let reader = BufReader::new(stdin);

    for line in reader.lines() {
        if let Ok(line) = line {
            match usize::from_str_radix(&line, 16) {
                Ok(address) =>
                    match MAP.get(&address) {
                        Some(backtrace) => println!("{:?}", *backtrace),
                        None => eprintln!("Can't find address: 0x{:X}", address),
                    },
                Err(error) => eprintln!("Can't parse address: {}", error),
            }
        }
        print!("> ");
        let _ = stdout.flush();
    }
}

fn handle_client(mut stream: TcpStream) {
    let mut message_size = None;
    let mut buffer = vec![];
    loop {
        let mut data = vec![0; 4096];
        match stream.read(&mut data) {
            Ok(size) => {
                //println!("Read: {}", size);
                if size == 0 {
                    return;
                }
                buffer.extend(&data[..size]);
            },
            Err(error) => {
                eprintln!("Read error: {}", error);
                continue;
            }
        }

        let mut msg_read = true;
        while msg_read && !buffer.is_empty() {
            msg_read = false;
            if message_size.is_none() {
                message_size = buf_to_u32(&buffer);
                if buffer.len() <= HEADER_SIZE {
                    buffer = vec![];
                }
                else {
                    buffer = buffer[HEADER_SIZE..].to_vec(); // TODO: avoid the copy.
                }
            }

            if let Some(msg_size) = message_size {
                let msg_size = msg_size as usize;
                if buffer.len() >= msg_size {
                    {
                        let mut decoder = Decoder::new(&buffer[..msg_size]);
                        match Decodable::decode(&mut decoder) {
                            Ok(msg) => {
                                message_size = None;
                                record(msg);
                            },
                            Err(error) => {
                                eprintln!("Failed to deserialize message: {:?}", error);
                            },
                        }
                    }
                    buffer = buffer[msg_size..].to_vec(); // TODO: avoid the copy.
                    msg_read = true;
                }
            }
        }
    }
}

fn buf_to_u32(buffer: &[u8]) -> Option<u32> {
    if buffer.len() >= HEADER_SIZE {
        Some(buffer[0] as u32 | (buffer[1] as u32) << 8 | (buffer[2] as u32) << 16 | (buffer[3] as u32) << 24)
    }
    else {
        None
    }
}

fn record(trace: Trace) {
    MAP.insert(trace.address, trace.backtrace);
}
