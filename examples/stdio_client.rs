use std::process::Stdio;

use tokio_tower::pipeline;
use tokio_util::codec::LinesCodec;
use tower::{Service, ServiceExt};
use tower_rpc::{
    codec_builder_fn, transport::stdio::StdioTransport, Client, Codec, CodecBuilder, SerdeCodec,
};

#[tokio::main]
pub async fn main() {
    let mut process = tokio::process::Command::new("cargo")
        .args(["run", "--example", "stdio_server"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut client = Client::new(tokio_util::codec::Framed::new(
        StdioTransport::from_child(&mut process),
        LinesCodec::default(),
    ))
    .create_pipeline();
    client.ready().await.unwrap();
    let a = client.call("test".to_owned()).await.unwrap();
    println!("RES {a:?}");
}
