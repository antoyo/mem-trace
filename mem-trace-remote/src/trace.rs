use backtrace::Backtrace;

pub const HEADER_SIZE: usize = 4;

#[derive(RustcDecodable, RustcEncodable)]
pub struct Trace {
    pub address: usize,
    pub backtrace: Backtrace,
}
