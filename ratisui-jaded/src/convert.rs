//! Traits and methods used to convert deserialized Java objects into useful
//! structures.
use crate::{ConversionError, ConversionResult, PrimitiveType, Value};

/// Trait for structs that can be converted from a deserialized Java Value
pub trait FromJava: Sized {
    /// Convert content read from a java stream into Self
    fn from_value(value: &Value) -> ConversionResult<Self>;
}

impl<T: FromJava> FromJava for Option<T> {
    fn from_value(value: &Value) -> ConversionResult<Self> {
        match value {
            Value::Null => Ok(None),
            _ => Ok(Some(T::from_value(value)?)),
        }
    }
}

impl FromJava for String {
    fn from_value(value: &Value) -> ConversionResult<Self> {
        match value {
            Value::JavaString(s) => Ok(s.to_string()),
            Value::Null => Err(ConversionError::NullPointerException),
            _ => Err(ConversionError::InvalidType("string")),
        }
    }
}

/// Implement FromJava for a primitive type. In Java a primitive can be a
/// 'real' primitive (int, double etc), or a boxed Object version
/// (java.lang.Integer, java.lang.Double) etc. This lets both variants be
/// converted to the equivalent rust types (i32, f64 etc).
macro_rules! from_value_for_primitive {
    // Optionally take a string literal as boxed object names don't always match
    // primitive name, eg int -> Integer
    ($type:ty, $primitive:ident) => {
        from_value_for_primitive! {$type, $primitive, stringify!($primitive)}
    };
    // Convert either a primitive or boxed primitive into its rust equivalent
    ($type:ty, $primitive:ident, $java_name:expr) => {
        impl FromJava for $type {
            fn from_value(value: &Value) -> ConversionResult<Self> {
                match value {
                    Value::Object(data) => {
                        let java_class_name = concat!("java.lang.", $java_name);
                        if data.class_name() == java_class_name {
                            match data.get_field("value") {
                                Some(Value::Primitive(PrimitiveType::$primitive(v))) => Ok(*v),
                                Some(_) => Err(ConversionError::InvalidType(stringify!($type))),
                                None => Err(ConversionError::FieldNotFound("value".to_string())),
                            }
                        } else {
                            Err(ConversionError::InvalidType(java_class_name))
                        }
                    }
                    Value::Primitive(PrimitiveType::$primitive(i)) => Ok(*i),
                    _ => Err(ConversionError::InvalidType($java_name)),
                }
            }
        }
    };
}

from_value_for_primitive!(u8, Byte);
from_value_for_primitive!(i16, Short);
from_value_for_primitive!(i32, Int, "Integer");
from_value_for_primitive!(i64, Long);
from_value_for_primitive!(f32, Float);
from_value_for_primitive!(f64, Double);
from_value_for_primitive!(char, Char, "Character");
from_value_for_primitive!(bool, Boolean);

impl<T: FromJava> FromJava for Box<T> {
    fn from_value(value: &Value) -> ConversionResult<Self> {
        Ok(Box::new(T::from_value(value)?))
    }
}

impl<T: FromJava> FromJava for Vec<T> {
    fn from_value(value: &Value) -> ConversionResult<Self> {
        match value {
            Value::Array(items) => Ok(items
                .iter()
                .map(T::from_value)
                .collect::<ConversionResult<Vec<_>>>()?),
            Value::PrimitiveArray(items) => Ok(items
                .iter()
                .map(|p| T::from_value(&Value::Primitive(*p)))
                .collect::<ConversionResult<Vec<_>>>()?),
            _ => Err(ConversionError::InvalidType("array")),
        }
    }
}
