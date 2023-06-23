use crate::AsyncReadWrite;
use futures::Stream;
pub use parity_tokio_ipc::SecurityAttributes;
use parity_tokio_ipc::{Connection, Endpoint};
use std::io;

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
    security_attributes: SecurityAttributes,
    #[allow(unused)] on_conflict: OnConflict,
) -> io::Result<impl Stream<Item = io::Result<impl AsyncReadWrite>> + 'static> {
    #[cfg(unix)]
    {
        let addr = get_socket_address(app_id.as_ref(), "");
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
    let mut endpoint = Endpoint::new(get_socket_address(app_id.as_ref(), ""));
    endpoint.set_security_attributes(security_attributes);
    endpoint.incoming()
}

pub async fn connect(app_id: impl AsRef<str>) -> io::Result<Connection> {
    Endpoint::connect(get_socket_address(app_id.as_ref(), "")).await
}
