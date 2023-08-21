use std::io;

use parity_tokio_ipc::{Connection, Endpoint, IpcEndpoint, IpcStream};
pub use parity_tokio_ipc::{
    ConnectionId, IntoIpcPath, IpcSecurity, OnConflict, SecurityAttributes,
};

pub fn create_endpoint(
    app_id: impl IntoIpcPath,
    security_attributes: SecurityAttributes,
    #[allow(unused)] on_conflict: OnConflict,
) -> io::Result<IpcStream> {
    let mut endpoint = Endpoint::new(app_id, on_conflict)?;
    endpoint.set_security_attributes(security_attributes);
    endpoint.incoming()
}

pub async fn connect(app_id: impl IntoIpcPath) -> io::Result<Connection> {
    Endpoint::connect(app_id).await
}
