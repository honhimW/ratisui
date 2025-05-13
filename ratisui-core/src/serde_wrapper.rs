use itertools::Itertools;
use deadpool_redis::redis::{PushKind, Value, VerbatimFormat};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use ron::ser::PrettyConfig;
use crate::utils::bytes_to_string;

pub fn to_ron_string(value: &Value) -> Result<String> {
    Ok(ron::ser::to_string_pretty(&IValue::from(value), PrettyConfig::default())?)
}

#[derive(Serialize, Deserialize)]
pub enum IValue {
    Nil,
    I(i64),
    BS(String),
    Arr(Vec<IValue>),
    S(String),
    Okay,
    Map(Vec<(IValue, IValue)>),
    Attr {
        data: Box<IValue>,
        attributes: Vec<(IValue, IValue)>,
    },
    Set(Vec<IValue>),
    F(f64),
    B(bool),
    VS {
        format: IVerbatimFormat,
        text: String,
    },
    BigNumber(String),
    Push {
        kind: IPushKind,
        data: Vec<IValue>,
    },
    ServerError(String),
}
#[derive(Serialize, Deserialize)]
pub enum IVerbatimFormat {
    /// Unknown type to catch future formats.
    Unknown(String),
    /// `mkd` format
    Markdown,
    /// `txt` format
    Text,
}
#[derive(Serialize, Deserialize)]
pub enum IPushKind {
    /// `Disconnection` is sent from the **library** when connection is closed.
    Disconnection,
    /// Other kind to catch future kinds.
    Other(String),
    /// `invalidate` is received when a key is changed/deleted.
    Invalidate,
    /// `message` is received when pubsub message published by another client.
    Message,
    /// `pmessage` is received when pubsub message published by another client and client subscribed to topic via pattern.
    PMessage,
    /// `smessage` is received when pubsub message published by another client and client subscribed to it with sharding.
    SMessage,
    /// `unsubscribe` is received when client unsubscribed from a channel.
    Unsubscribe,
    /// `punsubscribe` is received when client unsubscribed from a pattern.
    PUnsubscribe,
    /// `sunsubscribe` is received when client unsubscribed from a shard channel.
    SUnsubscribe,
    /// `subscribe` is received when client subscribed to a channel.
    Subscribe,
    /// `psubscribe` is received when client subscribed to a pattern.
    PSubscribe,
    /// `ssubscribe` is received when client subscribed to a shard channel.
    SSubscribe,
}

impl From<Value> for IValue {
    fn from(value: Value) -> Self {
        match value {
            Value::Nil => IValue::Nil,
            Value::Int(val) => IValue::I(val),
            Value::BulkString(val) => IValue::BS(bytes_to_string(val).unwrap_or("".to_string())),
            Value::Array(val) => IValue::Arr(val.iter().map(|x| IValue::from(x)).collect_vec()),
            Value::SimpleString(val) => IValue::S(val),
            Value::Okay => IValue::Okay,
            Value::Map(val) => IValue::Map(val.iter().map(|(x1, x2)| (IValue::from(x1), IValue::from(x2))).collect_vec()),
            Value::Attribute { data, attributes } => {
                IValue::Attr { data: Box::new(IValue::from(*data)), attributes: attributes.iter().map(|(x1, x2)| (IValue::from(x1), IValue::from(x2))).collect_vec() }
            },
            Value::Set(val) => IValue::Set(val.iter().map(|x| IValue::from(x)).collect_vec()),
            Value::Double(val) => IValue::F(val),
            Value::Boolean(val) => IValue::B(val),
            Value::VerbatimString { format, text } => IValue::VS { format: IVerbatimFormat::from(format), text },
            Value::BigNumber(val) => IValue::BigNumber(val.to_string()),
            Value::Push { kind, data } => IValue::Push { kind: IPushKind::from(kind), data: data.iter().map(|x| IValue::from(x)).collect_vec() },
            Value::ServerError(val) => IValue::ServerError(val.details().unwrap_or_default().to_string()),
        }
    }
}

impl From<&Value> for IValue {
    fn from(value: &Value) -> Self {
        let value = value.clone();
        Self::from(value)
    }
}

impl From<VerbatimFormat> for IVerbatimFormat {
    fn from(value: VerbatimFormat) -> Self {
        match value {
            VerbatimFormat::Unknown(val) => IVerbatimFormat::Unknown(val),
            VerbatimFormat::Markdown => IVerbatimFormat::Markdown,
            VerbatimFormat::Text => IVerbatimFormat::Text,
        }
    }
}

impl From<PushKind> for IPushKind {
    fn from(value: PushKind) -> Self {
        match value {
            PushKind::Disconnection => IPushKind::Disconnection,
            PushKind::Invalidate => IPushKind::Invalidate,
            PushKind::Message => IPushKind::Message,
            PushKind::PMessage => IPushKind::PMessage,
            PushKind::SMessage => IPushKind::SMessage,
            PushKind::Unsubscribe => IPushKind::Unsubscribe,
            PushKind::PUnsubscribe => IPushKind::PUnsubscribe,
            PushKind::SUnsubscribe => IPushKind::SUnsubscribe,
            PushKind::Subscribe => IPushKind::Subscribe,
            PushKind::PSubscribe => IPushKind::PSubscribe,
            PushKind::SSubscribe => IPushKind::SSubscribe,
            PushKind::Other(val) => IPushKind::Other(val),
        }
    }
}