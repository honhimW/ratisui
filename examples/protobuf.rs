#[path = "common/lib.rs"]
mod common;

use crate::common::client::dead_pool;
use anyhow::{anyhow, Result};
use protobuf::reflect::MessageDescriptor;
use protobuf::well_known_types::any::Any;
use protobuf::{CodedOutputStream, UnknownValueRef};
use redis::AsyncCommands;
use ron::ser::PrettyConfig;
use serde::Serialize;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {
    let pool = dead_pool()?;
    let mut connection = pool.get().await?;

    let protobuf: Vec<u8> = generate_bytes()?;
    connection.set("protobuf", protobuf).await?;
    let protobuf: Vec<u8> = connection.get("protobuf").await?;

    let descriptor = MessageDescriptor::for_type::<Any>();
    let dynamic_message = descriptor.parse_from_bytes(&protobuf)?;
    let any_message: Box<Any> = dynamic_message.downcast_box().map_err(|e| anyhow!(e))?;
    let mut map: HashMap<u32, Field> = HashMap::new();

    if !any_message.type_url.is_empty() {
        map.insert(1, Field::LengthDelimited(any_message.type_url.clone()));
    }
    if !any_message.value.is_empty() {
        map.insert(
            2,
            Field::LengthDelimited(String::from_utf8(any_message.value.clone().to_vec())?),
        );
    }

    let fields = any_message.special_fields;
    let unknown_fields = fields.unknown_fields();

    for (idx, unknown_field) in unknown_fields.iter() {
        println!("{}: {:?}", idx, unknown_field);
        match unknown_field {
            UnknownValueRef::Fixed32(fixed32) => map.insert(idx, Field::Fixed32(fixed32)),
            UnknownValueRef::Fixed64(fixed64) => map.insert(idx, Field::Fixed64(fixed64)),
            UnknownValueRef::Varint(varint) => map.insert(idx, Field::Varint(varint)),
            UnknownValueRef::LengthDelimited(ld) => {
                map.insert(idx, Field::LengthDelimited(String::from_utf8(ld.to_vec())?))
            }
        };
    }
    let string = ron::ser::to_string_pretty(&map, PrettyConfig::default())?;
    println!("{}", string);
    Ok(())
}

#[derive(Serialize)]
enum Field {
    Fixed32(u32),
    Fixed64(u64),
    Varint(u64),
    LengthDelimited(String),
}

fn generate_bytes() -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = vec![];
    {
        let mut stream = CodedOutputStream::vec(&mut buf);
        stream.write_string(5, "foo")?;
        stream.write_string(2, "bar")?;
        stream.write_int32(4, i32::MAX)?;
        stream.write_bool(1, true)?;
        stream.write_bool(7, true)?;
        stream.flush()?;
    }
    Ok(buf)
}
