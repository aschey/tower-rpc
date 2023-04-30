use std::{process::Stdio, time::Duration};

use tower::BoxError;
use tower_rpc::{serde_codec, transport::stdio::StdioTransport, Client, Codec, ReadyServiceExt};

#[tokio::main]
pub async fn main() -> Result<(), BoxError> {
    let mut process = tokio::process::Command::new("cargo")
        .args(["run", "--example", "stdio_server"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let client_stream =
        StdioTransport::from_child(&mut process).expect("Process missing io handles");
    let mut client =
        Client::new(serde_codec::<usize, usize>(client_stream, Codec::Bincode)).create_pipeline();
    let mut i = 0;

    loop {
        i = client.call_ready(i).await?;
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
