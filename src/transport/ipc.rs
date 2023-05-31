use futures::Stream;
use parity_tokio_ipc::{Connection, Endpoint};
use std::io;

use crate::AsyncReadWrite;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OnConflict {
    Ignore,
    Error,
    Overwrite,
}

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

pub fn create_endpoint(
    app_id: impl AsRef<str>,
    on_conflict: OnConflict,
) -> io::Result<impl Stream<Item = io::Result<impl AsyncReadWrite>> + 'static> {
    let addr = get_socket_address(app_id.as_ref(), "");
    #[cfg(unix)]
    {
        if std::path::Path::new(&addr).exists() {
            match on_conflict {
                OnConflict::Error => {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("Unable to bind to {addr} because the path already exists"),
                    ))
                }
                OnConflict::Overwrite => {
                    std::fs::remove_file(&addr)?;
                }
                OnConflict::Ignore => {}
            }
        }
    }
    Endpoint::new(get_socket_address(app_id.as_ref(), "")).incoming()
}

pub async fn connect(app_id: impl AsRef<str>) -> io::Result<Connection> {
    Endpoint::connect(get_socket_address(app_id.as_ref(), "")).await
}
