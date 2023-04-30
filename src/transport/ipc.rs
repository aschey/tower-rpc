use std::io;

use parity_tokio_ipc::{Connection, Endpoint};

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

pub fn create_endpoint(app_id: impl AsRef<str>) -> Endpoint {
    Endpoint::new(get_socket_address(app_id.as_ref(), ""))
}

pub async fn connect(app_id: impl AsRef<str>) -> io::Result<Connection> {
    Endpoint::connect(get_socket_address(app_id.as_ref(), "")).await
}
