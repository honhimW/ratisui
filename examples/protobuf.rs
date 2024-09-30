#[path = "common/lib.rs"]
mod common;

use std::collections::HashMap;
use std::io::Cursor;
use crate::common::client::dead_pool;
use anyhow::{Context, Result};
use futures::{StreamExt, TryStreamExt};
use itertools::Itertools;
use protobuf::reflect::MessageDescriptor;
use protobuf::rt::WireType;
use protobuf::UnknownValueRef;
use protobuf::well_known_types::any::Any;
use jaded::{Content, Parser};
use redis::{AsyncCommands, Value};
use ron::ser::PrettyConfig;
use serde::{Serialize, Serializer};

#[tokio::main]
async fn main() -> Result<()> {
    let pool = dead_pool()?;
    let mut connection = pool.get().await?;

    let protobuf: Vec<u8> = connection.get("protobuf").await?;

    let descriptor = MessageDescriptor::for_type::<Any>();
    let dynamic_message = descriptor.parse_from_bytes(&protobuf)?;
    let any_message: Box<Any> = dynamic_message.downcast_box().expect("");
    let fields = any_message.special_fields;
    let unknown_fields = fields.unknown_fields();
    #[derive(Serialize)]
    enum Field {
        Fixed32(u32),
        Fixed64(u64),
        Varint(u64),
        LengthDelimited(String)
    }

    let mut map: HashMap<u32, Field> = HashMap::new();

    for (idx, unknown_field) in unknown_fields.iter() {
        match unknown_field {
            UnknownValueRef::Fixed32(fixed32) => map.insert(idx, Field::Fixed32(fixed32)),
            UnknownValueRef::Fixed64(fixed64) => map.insert(idx, Field::Fixed64(fixed64)),
            UnknownValueRef::Varint(varint) => map.insert(idx, Field::Varint(varint)),
            UnknownValueRef::LengthDelimited(ld) => map.insert(idx, Field::LengthDelimited(String::from_utf8(ld.to_vec())?)),
        };
    }
    let string = ron::ser::to_string_pretty(&map, PrettyConfig::default())?;
    println!("{}", string);
    Ok(())

}

