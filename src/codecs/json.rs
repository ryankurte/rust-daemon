/**
 * rust-daemon
 * JSON Codec
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */

use std::io;
use std::marker::PhantomData;

use bytes::{BufMut, BytesMut};
use tokio_io::_tokio_codec::{Encoder, Decoder};
use serde::{Serialize, Deserialize};
use serde_json;

/// A codec for JSON encoding and decoding
/// ENC is the type to encode, DEC is the type to decode, ERR is the error type to be
/// returned for both operations
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct JsonCodec<ENC, DEC, ERR> 
{
    enc: PhantomData<ENC>,
    dec: PhantomData<DEC>,
    err: PhantomData<ERR>,
}

/// Basic compatible error type
#[derive(Debug)]
pub enum JsonError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl From<io::Error> for JsonError {
    fn from(e: io::Error) -> JsonError {
        return JsonError::Io(e);
    }
}

impl From<serde_json::Error> for JsonError {
    fn from(e: serde_json::Error) -> JsonError {
        return JsonError::Json(e);
    }
}

/// New builds an empty codec with associated types
impl <ENC, DEC, ERR>JsonCodec<ENC, DEC, ERR> 
where 
    for<'de> DEC: Deserialize<'de> + Clone + Send + 'static,
    for<'de> ENC: Serialize + Clone + Send + 'static,
    ERR: From<serde_json::Error> + From<io::Error> + 'static,
{
    /// Creates a new `JsonCodec` for shipping around raw bytes.
    pub fn new() -> JsonCodec<ENC, DEC, ERR> { 
        JsonCodec {enc: PhantomData, dec: PhantomData, err: PhantomData}  
    }
}

/// Default impl required for use with connections
impl <ENC, DEC, ERR>Default for JsonCodec<ENC, DEC, ERR> 
where 
    for<'de> DEC: Deserialize<'de> + Clone + Send + 'static,
    for<'de> ENC: Serialize + Clone + Send + 'static,
    ERR: From<serde_json::Error> + From<io::Error> + 'static,
{
    fn default() -> JsonCodec<ENC, DEC, ERR> {
        JsonCodec::new()
    }
}

/// Decoder impl parses json objects from bytes
impl <ENC, DEC, ERR>Decoder for JsonCodec<ENC, DEC, ERR> 
where 
    for<'de> DEC: Deserialize<'de> + Clone + Send + 'static,
    for<'de> ENC: Serialize + Clone + Send + 'static,
    ERR: From<serde_json::Error> + From<io::Error> + 'static,
{
    type Item = DEC;
    type Error = ERR;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let offset;
        let res;
        
        {
            // Build streaming JSON iterator over data
            let de = serde_json::Deserializer::from_slice(&buf);
            let mut iter = de.into_iter::<DEC>();

            // Attempt to fetch an item and generate response
            res = match iter.next() {
                Some(Ok(v)) => Ok(Some(v)),
                Some(Err(ref e)) if e.is_eof() => {
                    Ok(None)
                },
                Some(Err(e)) => Err(e.into()),
                None => Ok(None),
            };
            offset = iter.byte_offset();
        }

        // Advance buffer
        buf.advance(offset);

        res
    }
}

/// Encoder impl encodes object streams to bytes
impl <ENC, DEC, ERR>Encoder for JsonCodec<ENC, DEC, ERR> 
where 
    for<'de> DEC: Deserialize<'de> + Clone + Send + 'static,
    for<'de> ENC: Serialize + Clone + Send + 'static,
    ERR: From<serde_json::Error> + From<io::Error> + 'static,
{
    type Item = ENC;
    type Error = ERR;

    fn encode(&mut self, data: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        // Encode json
        let j = serde_json::to_string(&data)?;
        
        // Write to buffer
        buf.reserve(j.len());
        buf.put_slice(&j.as_bytes());

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;
    use tokio_codec::{Encoder, Decoder};

    use super::{JsonCodec, JsonError};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestStruct {
        pub name: String,
    }

    #[test]
    fn json_codec_encode_decode() {
        let mut codec = JsonCodec::<TestStruct, TestStruct, JsonError>::new();
        let mut buff = BytesMut::new();

        let item1 = TestStruct{name: "Test name".to_owned()};
        codec.encode(item1.clone(), &mut buff).unwrap();

        let item2 = codec.decode(&mut buff).unwrap().unwrap();
        assert_eq!(item1, item2);

        assert_eq!(codec.decode(&mut buff).unwrap(), None);

        assert_eq!(buff.len(), 0);
    }

    #[test]
    fn json_codec_partial_decode() {
        let mut codec = JsonCodec::<TestStruct, TestStruct, JsonError>::new();
        let mut buff = BytesMut::new();

        let item1 = TestStruct{name: "Test name".to_owned()};
        codec.encode(item1.clone(), &mut buff).unwrap();

        let mut start = buff.clone().split_to(4);
        assert_eq!(codec.decode(&mut start).unwrap(), None);

        codec.decode(&mut buff).unwrap().unwrap();

        assert_eq!(buff.len(), 0);
        
    }
}