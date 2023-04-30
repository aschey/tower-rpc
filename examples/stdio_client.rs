use std::{process::Stdio, time::Duration};

use tower_rpc::{serde_codec, transport::stdio::StdioTransport, Client, Codec, ReadyServiceExt};

#[tokio::main]
pub async fn main() {
    let mut process = tokio::process::Command::new("cargo")
        .args(["run", "--example", "stdio_server"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let client_stream = StdioTransport::from_child(&mut process);
    let mut client =
        Client::new(serde_codec::<usize, usize>(client_stream, Codec::Bincode)).create_pipeline();
    let mut i = 0;

    loop {
        i = client.call_ready(i).await.unwrap();
        println!("Pong {i}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
