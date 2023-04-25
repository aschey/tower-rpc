use serde::{Deserialize, Serialize};
use std::io;
use std::marker::PhantomData;
use std::pin::Pin;
#[cfg(feature = "bincode")]
use tokio_serde::formats::Bincode;
#[cfg(feature = "cbor")]
use tokio_serde::formats::Cbor;
#[cfg(feature = "json")]
use tokio_serde::formats::Json;
#[cfg(feature = "messagepack")]
use tokio_serde::formats::MessagePack;
use tokio_serde::{Deserializer, Serializer};

use crate::codec::Codec;

pub struct CodecWrapper<Item, SinkItem>
where
    SinkItem: Serialize + Unpin,
    Item: for<'de> Deserialize<'de> + Unpin,
{
    codec: Codec,
    phantom: PhantomData<(Item, SinkItem)>,
}

impl<Item, SinkItem> CodecWrapper<Item, SinkItem>
where
    SinkItem: Serialize + Unpin,
    Item: for<'de> Deserialize<'de> + Unpin,
{
    pub fn new(codec: Codec) -> Self {
        Self {
            codec,
            phantom: Default::default(),
        }
    }
}

impl<Item, SinkItem> Serializer<SinkItem> for CodecWrapper<Item, SinkItem>
where
    SinkItem: Serialize + Unpin,
    Item: for<'de> Deserialize<'de> + Unpin,
{
    type Error = io::Error;

    fn serialize(self: Pin<&mut Self>, item: &SinkItem) -> Result<bytes::Bytes, Self::Error> {
        match self.codec {
            #[cfg(feature = "bincode")]
            Codec::Bincode => Pin::new(&mut Bincode::<Item, SinkItem>::default()).serialize(item),
            #[cfg(feature = "json")]
            Codec::Json => Pin::new(&mut Json::<Item, SinkItem>::default())
                .serialize(item)
                .map_err(|e| e.into()),
            #[cfg(feature = "messagepack")]
            Codec::MessagePack => {
                Pin::new(&mut MessagePack::<Item, SinkItem>::default()).serialize(item)
            }
            #[cfg(feature = "cbor")]
            Codec::Cbor => Pin::new(&mut Cbor::<Item, SinkItem>::default()).serialize(item),
        }
    }
}

impl<Item, SinkItem> Deserializer<Item> for CodecWrapper<Item, SinkItem>
where
    SinkItem: Serialize + Unpin,
    Item: for<'de> Deserialize<'de> + Unpin,
{
    type Error = io::Error;

    fn deserialize(self: Pin<&mut Self>, src: &bytes::BytesMut) -> Result<Item, Self::Error> {
        match self.codec {
            #[cfg(feature = "bincode")]
            Codec::Bincode => Pin::new(&mut Bincode::<Item, SinkItem>::default()).deserialize(src),
            #[cfg(feature = "json")]
            Codec::Json => Pin::new(&mut Json::<Item, SinkItem>::default())
                .deserialize(src)
                .map_err(|e| e.into()),
            #[cfg(feature = "messagepack")]
            Codec::MessagePack => {
                Pin::new(&mut MessagePack::<Item, SinkItem>::default()).deserialize(src)
            }
            #[cfg(feature = "cbor")]
            Codec::Cbor => Pin::new(&mut Cbor::<Item, SinkItem>::default()).deserialize(src),
        }
    }
}
