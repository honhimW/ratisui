use std::{borrow::Cow, string::FromUtf8Error};
use thiserror::Error;

/// Overall error type for everything that can go wrong with
/// Java deserialization
#[derive(Debug, Error)]
pub enum JavaError {
    /// An error in the read/stream/deserialization process
    #[error("Error read from stream")]
    ReadError(#[from] StreamError),
    /// An error arising from the conversion of read data to Rust struct
    #[error("Error converting read object into something useful: {0}")]
    ConvertError(#[from] ConversionError),
}

/// Error for things that can go wrong with deserialization
#[derive(Debug, Error)]
pub enum StreamError {
    /// If the stream ends while the parser is still expecting more data
    #[error("Unexpected end of stream")]
    EndOfStream(#[from] std::io::Error),
    /// If the stream does not represent Serialized Java objects
    #[error("This isn't a JavaObject - magic numbers: {0:X}")]
    NonJavaObject(u16),
    /// If the stream version is not one we can handle
    #[error("Unknown serialization version: {0}")]
    UnknownVersion(u16),
    /// If the next stream marker is not a value recognised by the serialization
    /// protocol
    ///
    /// The byte read from the stream will be included in the error.
    #[error("Unknown mark: {0}")]
    UnknownMark(u8),
    /// If the type or a field is not one of the allowed characters
    ///
    /// The character being read as a type specification is included in the error
    #[error("Unknown type marker: {0}")]
    UnrecognisedType(char),
    /// If a back reference is to an unregistered handle
    ///
    /// The unrecognised handle is included in the error
    #[error("Unknown reference handle: {0}")]
    UnknownReference(u32),
    /// If the reference registered to a handle is not of the correct type
    ///
    /// The included string is the type that was expected, not the type found.
    #[error("Invalid reference. Expected {0}")]
    InvalidReference(&'static str),
    /// The stream is not valid for some other reason
    ///
    /// The included string gives an error message of the problem found.
    #[error("Invalid Stream: {0}")]
    InvalidStream(&'static str),
    /// This feature is not implemented yet
    /// Some features are not possible without access to the Java source that
    /// wrote the stream.
    #[error("{0} isn't implemented yet")]
    NotImplemented(&'static str),
}

impl From<FromUtf8Error> for StreamError {
    fn from(_: FromUtf8Error) -> Self {
        StreamError::InvalidStream("String is not valid UTF-8")
    }
}

/// Things that can go wrong when converting deserialized java objects into
/// Rust entities
#[derive(Debug, Error)]
pub enum ConversionError {
    /// Error signifying a required field does not exist in the deserialized
    /// Java Object
    #[error("Field '{0}' does not exist")]
    FieldNotFound(String),
    /// Everyone's favourite Java exception brought to rust. Returned if a
    /// non-optional field has a null value.
    #[error("No object found")]
    NullPointerException,
    /// The deserialized object was not the type required.
    #[error("Expected '{0}'")]
    InvalidType(&'static str),
    /// Read block data instead of object
    #[error("Unexpected block data")]
    UnexpectedBlockData(Vec<u8>),
    /// Annotation not available
    #[error("Missing Annotation: {0}")]
    MissingAnnotations(usize),
    /// Incorrect class
    #[error("Expected class '{0}', found '{1}'")]
    IncorrectClass(Cow<'static, str>, Cow<'static, str>),
    /// Unmatched class for enum
    #[error("Class '{0}' was not expected")]
    UnexpectedClass(String),
}
