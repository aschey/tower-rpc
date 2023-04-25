mod client_stream;
pub use client_stream::*;
mod transport;
pub use transport::*;

pub fn get_socket_address(id: &str, suffix: &str) -> String {
    let suffix_full = if suffix.is_empty() {
        "".to_owned()
    } else {
        format!("_{suffix}")
    };

    #[cfg(unix)]
    let addr = format!("/tmp/{id}{suffix_full}.sock");
    #[cfg(windows)]
    let addr = format!("\\\\.\\pipe\\{id}{suffix_full}");
    addr
}
