use std::process::Stdio;
use std::time::Duration;

use tower::{BoxError, Service, ServiceExt};
use tower_rpc::transport::codec::{serde_codec, Codec};
use tower_rpc::transport::stdio::StdioTransport;
use tower_rpc::Client;

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let mut process = tokio::process::Command::new("cargo")
        .args(["run", "--example", "stdio_server", "--all-features"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let client_stream =
        StdioTransport::from_child(&mut process).expect("Process missing io handles");
    let mut client =
        Client::new(serde_codec::<usize, usize>(client_stream, Codec::Bincode)).create_pipeline();
    let mut i = 0;

    loop {
        i = client.ready().await?.call(i).await?;
        eprintln!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
