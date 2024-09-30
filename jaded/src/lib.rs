//! # Java Deserializer for Rust
//!
//! Java has a much maligned but still widely used Serialization mechanism.
//! The serial stream produced follows the specification available from Oracle
//! [here](https://docs.oracle.com/en/java/javase/17/docs/specs/serialization/protocol.html)
//! (link is to Java 17 (latest LTS version at time of writing) but protocol
//! hasn't changed since 1.7).
//!
//! This library enables that serial stream to be read in Rust applications.
//!
//! ## Example
//! A simple bean type Java class can be serialized using the builtin tools
//!
//! ```java
//! import java.io.FileOutputStream;
//! import java.io.ObjectOutputStream;
//! import java.io.Serializable;
//! public class Demo implements Serializable {
//!     private static final long serialVersionUID = 1L;
//!     private String message;
//!     private int i;
//!     public Demo(String message, int count) {
//!         this.message = message;
//!         this.i = count;
//!     }
//!     public static void main(String[] args) throws Exception {
//!         Demo d = new Demo("helloWorld", 42);
//!         try (FileOutputStream fos = new FileOutputStream("demo.obj", false);
//!                 ObjectOutputStream oos = new ObjectOutputStream(fos);) {
//!             oos.writeObject(d);
//!         }
//!     }
//! }
//! ```
//!
//! We can read in the `demo.obj` file written as a rust struct
//! ```no_run
//! # use std::fs::File;
//! # use jaded::{Parser, Result, Value::{JavaString, Primitive}, PrimitiveType::Int};
//! # fn main() -> Result<()> {
//! // Open the file written in Java
//! let sample = File::open("demo.obj").expect("File missing");
//! // Create a new parser to wrap the file
//! let mut parser = Parser::new(sample)?;
//! // read an object from the stream
//! let content = parser.read()?;
//! // the content read was a value (instead of raw data) and the value
//! // was an instance of an object. These methods would panic if the content
//! // was of a different type (equivalent to Option#unwrap).
//! let demo = content.value().object_data();
//! assert_eq!("Demo", demo.class_name());
//! assert_eq!(Some(JavaString("helloWorld".to_string())).as_ref(), demo.get_field("message"));
//! assert_eq!(Some(Primitive(Int(42))).as_ref(), demo.get_field("i"));
//! # Ok(())
//! # }
//! ```
//! Reading into the raw data format is not often very user friendly so rust
//! types can be read directly from the stream if they implement [`FromJava`].
//!
//! This is implemented for the primitive types and common types (boxed primitives,
//! arrays, String). Custom types can use these to implement `FromJava`
//! themselves.
//!
//! For example to read a string from an object stream
//! ```ignore
//! let mut parser = Parser::new(sample)?;
//! let string: String = parser.read_as()?;
//! ```
//! Implemententing `FromJava` for custom types is very repetitive so a
//! [`derive`](jaded_derive) macro is provided with the `derive` feature to
//! automatically generate implementations. Using the same `Demo` class from
//! above this gives us
//!
//! ```ignore
//! #[derive(Debug, FromJava)]
//! struct Demo {
//!     message: String,
//!     i: i32,
//! }
//!
//! let mut parser = Parser::new(sample)?;
//! let demo: Demo = parser.read_as()?;
//! println!("{:#?}", demo);
//!
//! // Output
//! // Demo {
//! //     message: "helloWorld",
//! //     i: 42,
//! // }
//! ```
//!

#![warn(missing_docs)]
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::{Display, Formatter};
use std::io::Read;
use std::mem::replace;
mod annotations;
mod convert;
mod error;
mod internal;
pub use annotations::AnnotationIter;
pub use convert::FromJava;
pub use error::{ConversionError, JavaError, StreamError};
use internal::{Flag, JavaStream, Marker};

#[cfg(feature = "derive")]
pub use jaded_derive::FromJava;

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

/// Result type for all deserialization operations
pub type Result<T> = std::result::Result<T, JavaError>;

/// Result type for deserialization process
pub type ReadResult<T> = std::result::Result<T, StreamError>;

/// Result type for conversion process
pub type ConversionResult<T> = std::result::Result<T, ConversionError>;

/// Handle are integers but make it explict what functions expect.
type Handle = u32;

/// The first two bytes of any stream to mark it as Java Serialized Object data
const MAGIC: u16 = 0xAC_ED;
/// The serialization protocol expected
const VERSION: u16 = 0x00_05;

/// The handle we assign to null values.
/// The Java protocol does not assign handles to null objects but it is useful
/// to treat all objects in the same way within the parsed data.
const NULL_HANDLE: Handle = 0;

/// The handle given to the first object read. Subsequent objects take
/// successive handle values
const INITIAL_HANDLE: u32 = 0x7E_00_00; // Because why not start there

/// The main parser class
/// ```
/// # use std::fs::File;
/// # use jaded::Parser;
/// # fn main() -> jaded::Result<()> {
/// // Input can be anything that implements Read, eg file or tcp stream
/// let sample = File::open("sample").expect("Sample file missing");
/// // Stream is checked at creation to ensure stream is java serialization
/// let mut parser = Parser::new(sample)?;
/// // Objects are read from stream in turn
/// let obj = parser.read()?;
/// println!("Read Object: {:?}", obj);
/// println!("Class name: {}", obj.value().object_data().class_name());
/// # Ok(())
/// # }
/// ```
pub struct Parser<T: Read> {
    /// The source stream
    stream: Box<T>,
    /// Map of previous objects to support lookup of backreferences
    references: HashMap<Handle, Reference>,
    /// The next handle to be assigned
    handle: Handle,
    /// The handles of all objects currently being resolved
    handle_stack: Vec<Handle>,
    /// Stream protocol version. Currently always 5
    _version: u16,
}

impl<T: Read> Parser<T> {
    /// Create a parser to read Java objects from a serial stream
    pub fn new(mut source: T) -> Result<Self> {
        let magic = source.read_u16()?;
        let version = source.read_u16()?;
        match (magic, version) {
            (MAGIC, VERSION) => Ok(Self {
                stream: Box::new(source),
                _version: version,
                handle_stack: vec![],
                handle: INITIAL_HANDLE,
                references: HashMap::new(),
            }),
            (MAGIC, v) => Err(JavaError::ReadError(StreamError::UnknownVersion(v))),
            (m, _) => Err(JavaError::ReadError(StreamError::NonJavaObject(m))),
        }
    }

    /// Get the next handle (u32) to assign. Gets and increments the
    /// internal counter and adds it to the stack of objects currently being
    /// deserialized. For nested objects, this allows self referencing fields.
    fn next_handle(&mut self) -> Handle {
        let next = self.handle + 1;
        self.handle_stack.push(self.handle);
        replace(&mut self.handle, next)
    }

    fn read_from_stream(&mut self) -> ReadResult<StreamRecord> {
        use StreamRecord::*;
        Ok(match self.stream.read_marker()? {
            Marker::Null => Ref(NULL_HANDLE),
            Marker::Reference => Ref(self.read_reference()?),
            Marker::Object => Ref(self.read_object()?),
            Marker::Array => Ref(self.read_array()?),
            Marker::Enum => Ref(self.read_enum()?),
            Marker::ClassDesc => Ref(self.read_class_desc()?),
            Marker::ProxyClassDesc => Ref(self.read_proxy_class()?),
            Marker::JavaString => Ref(self.read_java_string()?),
            Marker::LongString => Ref(self.read_long_string()?),
            Marker::Class => Ref(self.read_class()?), // This is a class that has been serialized, eg String.class
            Marker::BlockData => BlockData(self.read_block_data()?),
            Marker::BlockDataLong => BlockData(self.read_block_data_long()?),
            Marker::EndBlockData => EndBlockData,
            // This is an exception in the
            // serialization process not an exception that has been serialized
            Marker::Exception => Ref(self.read_exception()?),
            Marker::Reset => {
                self.reset();
                self.read_from_stream()? // potential for recursion problems for a stream of resets?
            }
        })
    }

    fn read_reference(&mut self) -> ReadResult<Handle> {
        self.stream.read_u32()
    }

    fn read_class_desc(&mut self) -> ReadResult<Handle> {
        let class_name = self.stream.read_string()?;
        let serial_uid = self.stream.read_u64()?;
        let handle = self.next_handle();
        let flags = self.stream.read_u8()?.try_into()?;
        let field_count = self.stream.read_u16()?;
        let mut fields = Vec::with_capacity(field_count as usize);
        for _ in 0..field_count {
            let type_code = self.stream.read_u8()? as char;
            let field_name = self.stream.read_string()?;
            fields.push(if type_code == 'L' || type_code == '[' {
                match self.read_from_stream()? {
                    StreamRecord::Ref(_) => FieldSpec {
                        name: field_name,
                        type_spec: type_code,
                    },
                    _ => return Err(StreamError::InvalidReference("Expected reference handle")),
                }
            } else {
                FieldSpec {
                    name: field_name,
                    type_spec: type_code,
                }
            });
        }
        let annotations = self.read_annotations()?;
        let super_class = match self.read_from_stream()? {
            StreamRecord::Ref(NULL_HANDLE) => None,
            StreamRecord::Ref(hnd) => Some(hnd),
            _ => {
                return Err(StreamError::InvalidStream(
                    "Super class is neither NewClassDesc nor null",
                ))
            }
        };
        let class_outline = ClassOutline {
            class_name,
            fields,
            _serial_uid: serial_uid,
            _annotations: annotations,
            super_class,
            flags,
        };
        self.register(&handle, Reference::Class(class_outline));
        Ok(handle)
    }

    fn read_proxy_class(&mut self) -> ReadResult<Handle> {
        let handle = self.next_handle();
        let interface_count = self.stream.read_u32()? as i32;
        let mut interfaces = vec![];
        for _ in 0..interface_count {
            interfaces.push(self.stream.read_string()?);
        }
        let annotations = self.read_annotations()?;
        let class_outline = *self.read_from_stream()?.handle()?;
        let proxy = Reference::Proxy(ProxyOutline {
            _interfaces: interfaces,
            _annotations: annotations,
            class_outline,
        });
        self.register(&handle, proxy);
        Ok(handle)
    }

    fn read_annotations(&mut self) -> ReadResult<Vec<Annotation>> {
        let mut annotations = vec![];
        use Annotation as A;
        use StreamRecord::*;
        loop {
            match self.read_from_stream()? {
                Ref(hnd) => annotations.push(A::Ref(hnd)),
                BlockData(data) => annotations.push(A::Block(data)),
                EndBlockData => break,
            };
        }
        Ok(annotations)
    }

    fn read_object(&mut self) -> ReadResult<Handle> {
        let class_desc = self.read_from_stream()?;
        let handle = self.next_handle();
        let order = self.build_read_list(class_desc.handle()?)?;
        let mut fields = HashMap::new();
        let mut annotations = vec![];
        for read in order {
            match read {
                ReadOrder::Fields(spec) => {
                    for f in spec {
                        fields.insert(f.name, self.read_value(f.type_spec)?);
                    }
                }
                ReadOrder::Annotations => annotations.push(self.read_annotations()?),
            }
        }
        let new_obj = Reference::Object(JavaObject {
            class: *class_desc.handle()?,
            fields,
            annotations,
        });
        self.register(&handle, new_obj);
        Ok(handle)
    }

    fn build_read_list(&mut self, hnd: &Handle) -> ReadResult<Vec<ReadOrder>> {
        let class_desc = self.class_from(hnd)?;
        // If any superclass in the hierarchy is externalizable, all subclasses
        // are as well, and no default fields are written. Just read block data
        if class_desc.flags == Flag::ExtBlock {
            return Ok(vec![ReadOrder::Annotations]);
        }
        // otherwise, climb the class hierarchy and then add fields on the way down
        let mut order = match class_desc.super_class {
            Some(h) => self.build_read_list(&h)?,
            None => vec![],
        };
        order.push(ReadOrder::Fields(class_desc.fields));
        if class_desc.flags == Flag::Write {
            order.push(ReadOrder::Annotations);
        }
        Ok(order)
    }

    fn read_java_string(&mut self) -> ReadResult<Handle> {
        let handle = self.next_handle();
        let text = self.stream.read_string()?;
        self.register(&handle, Reference::JavaString(text));
        Ok(handle)
    }

    fn read_long_string(&mut self) -> ReadResult<Handle> {
        let handle = self.next_handle();
        let text = self.stream.read_long_string()?;
        self.register(&handle, Reference::JavaString(text));
        Ok(handle)
    }

    fn read_array(&mut self) -> ReadResult<Handle> {
        let class_handle = self.read_from_stream()?;
        let handle = self.next_handle();
        let len = self.stream.read_u32()? as i32;

        let new_array = match self
            .class_from(class_handle.handle()?)?
            .class_name
            .chars()
            .nth(1)
        {
            Some('L') | Some('[') => {
                // Arrays and Objects are stored as references
                let mut data = vec![];
                for _ in 0..len {
                    data.push(*self.read_from_stream()?.handle()?);
                }
                Reference::Array(data)
            }
            Some(x) => {
                // primitives are stored as values in PrimitiveType
                let mut data = vec![];
                for _ in 0..len {
                    data.push(self.read_primitive(x)?);
                }
                Reference::PrimitiveArray(data)
            }
            None => return Err(StreamError::UnrecognisedType(' ')),
        };
        self.register(&handle, new_array);
        Ok(handle)
    }

    fn read_value(&mut self, type_code: char) -> ReadResult<Field> {
        use Field::*;
        Ok(match type_code {
            'L' | '[' => match self.read_from_stream()? {
                StreamRecord::Ref(obj_handle) => match self.get_from_handle(&obj_handle) {
                    Ok(_) => Field::Reference(obj_handle),
                    Err(_) => Field::Loop(
                        self.handle_stack
                            .iter()
                            .position(|i| i == &obj_handle)
                            .ok_or(StreamError::UnknownReference(obj_handle))?
                            as u32,
                    ),
                },
                _ => return Err(StreamError::InvalidReference("Object")),
            },
            c => Primitive(self.read_primitive(c)?),
        })
    }

    fn read_primitive(&mut self, type_code: char) -> ReadResult<PrimitiveType> {
        use PrimitiveType::*;
        Ok(match type_code {
            'C' => {
                let c = self.stream.read_u16()? as u32;
                Char(
                    std::char::from_u32(c)
                        .ok_or(StreamError::InvalidStream("invalid character"))?,
                )
            }
            'B' => Byte(self.stream.read_u8()?),
            'S' => Short(self.stream.read_u16()? as i16),
            'I' => Int(self.stream.read_u32()? as i32),
            'J' => Long(self.stream.read_u64()? as i64),
            'F' => Float(f32::from_bits(self.stream.read_u32()?)),
            'D' => Double(f64::from_bits(self.stream.read_u64()?)),
            'Z' => Boolean(self.stream.read_u8()? == 1),
            x => return Err(StreamError::UnrecognisedType(x)),
        })
    }

    fn read_class(&mut self) -> ReadResult<Handle> {
        let desc = self.read_from_stream()?;
        let handle = self.next_handle();
        let new_class = Reference::ClassObject(*desc.handle()?);
        self.register(&handle, new_class);
        Ok(handle)
    }

    fn read_enum(&mut self) -> ReadResult<Handle> {
        let class_desc = self.read_from_stream()?;
        let handle = self.next_handle();
        let constant_name = self.read_from_stream()?;
        let new_enum = Reference::Enum(*class_desc.handle()?, *constant_name.handle()?);
        self.register(&handle, new_enum);
        Ok(handle)
    }

    fn read_block_data(&mut self) -> ReadResult<Vec<u8>> {
        let len = self.stream.read_u8()?;
        let mut data = vec![];
        for _ in 0..len {
            data.push(self.stream.read_u8()?);
        }
        Ok(data)
    }

    fn reset(&mut self) {
        self.handle = INITIAL_HANDLE;
        self.references.clear();
    }

    fn read_block_data_long(&mut self) -> ReadResult<Vec<u8>> {
        let len = self.stream.read_u32()?;
        let mut data = vec![];
        for _ in 0..len {
            data.push(self.stream.read_u8()?);
        }
        Ok(data)
    }

    fn read_exception(&mut self) -> ReadResult<Handle> {
        self.reset();
        let exception = *self.read_from_stream()?.handle()?;
        self.reset();
        Ok(exception)
    }

    fn get_from_handle(&self, handle: &u32) -> ReadResult<&Reference> {
        if handle == &NULL_HANDLE {
            Ok(&Reference::Null)
        } else {
            match self.references.get(handle) {
                Some(refn) => Ok(refn),
                None => Err(StreamError::UnknownReference(*handle)),
            }
        }
    }

    fn register(&mut self, handle: &u32, reference: Reference) {
        if self.handle_stack.pop() != Some(*handle) {
            panic!("object was registered before something it references");
        }
        self.references.insert(*handle, reference);
    }

    fn class_from(&self, hnd: &Handle) -> ReadResult<ClassOutline> {
        let reference = self.get_from_handle(hnd)?;
        match reference {
            Reference::Proxy(proxy) => Ok(self.class_from(&proxy.class_outline)?),
            Reference::Class(outline) => Ok(outline.clone()),
            _ => Err(StreamError::InvalidReference("Class")),
        }
    }

    fn value_from_reference(&self, hnd: &Handle) -> ReadResult<Value> {
        use Reference::*;
        use Value as V;
        Ok(match self.get_from_handle(hnd)? {
            Null => V::Null,
            PrimitiveArray(data) => V::PrimitiveArray(data.to_vec()),
            Array(data) => V::Array(
                data.iter()
                    .map(|h| self.value_from_reference(h))
                    .collect::<ReadResult<_>>()?,
            ),
            JavaString(s) => V::JavaString(s.to_string()),
            Enum(class, cons) => {
                let outline = self.get_from_handle(class)?.class_outline()?;
                let constant = self.get_from_handle(cons)?.as_string()?;
                V::Enum(outline.class_name.to_string(), constant.to_string())
            }
            ClassObject(h) => V::Class(
                self.get_from_handle(h)?
                    .class_outline()?
                    .class_name
                    .to_string(),
            ),
            Object(obj) => {
                let class = self.class_from(&obj.class)?;
                let mut field_data = HashMap::new();
                for (name, field) in &obj.fields {
                    field_data.insert(
                        name.to_string(),
                        match field {
                            Field::Loop(l) => Value::Loop(-(*l as i32)),
                            Field::Reference(h) => self.value_from_reference(h)?,
                            Field::Primitive(p) => Value::Primitive(*p),
                        },
                    );
                }
                let mut anno_data = vec![];
                for class_anno in &obj.annotations {
                    anno_data.push(
                        class_anno
                            .iter()
                            .map(|anno| {
                                Ok(match anno {
                                    Annotation::Ref(h) => {
                                        Content::Object(self.value_from_reference(h)?)
                                    }
                                    Annotation::Block(data) => Content::Block(data.to_vec()),
                                })
                            })
                            .collect::<ReadResult<Vec<_>>>()?,
                    );
                }
                V::Object(ObjectData {
                    class: class.class_name,
                    fields: field_data,
                    annotations: anno_data,
                })
            }
            _ => return Err(StreamError::NotImplemented("value from reference")),
        })
    }

    /// Read the next item from the Java stream
    pub fn read(&mut self) -> ReadResult<Content> {
        use StreamRecord::*;
        Ok(match self.read_from_stream()? {
            Ref(hnd) => Content::Object(self.value_from_reference(&hnd)?),
            EndBlockData => return Err(StreamError::InvalidStream("Unexpected EndBlockData mark")),
            BlockData(data) => Content::Block(data),
        })
    }

    /// Read the next item from the stream and convert it to required type
    pub fn read_as<S: FromJava>(&mut self) -> Result<S> {
        match self.read()? {
            Content::Object(value) => Ok(S::from_value(&value)?),
            Content::Block(data) => Err(JavaError::ConvertError(
                ConversionError::UnexpectedBlockData(data),
            )),
        }
    }
}

#[derive(Debug)]
enum StreamRecord {
    /// End of raw byte stream
    EndBlockData,
    /// Stream of raw bytes in stream
    BlockData(Vec<u8>),
    /// Reference to previous data
    Ref(u32),
}

impl StreamRecord {
    fn handle(&self) -> ReadResult<&Handle> {
        match self {
            StreamRecord::Ref(hnd) => Ok(hnd),
            _ => Err(StreamError::InvalidReference("Ref")),
        }
    }
}

#[derive(Debug)]
enum Reference {
    Class(ClassOutline),
    Proxy(ProxyOutline),
    Null,
    JavaString(String),
    Array(Vec<Handle>),
    PrimitiveArray(Vec<PrimitiveType>),
    ClassObject(Handle), // eg from from serializing String.class handle -> Class(outline)
    Object(JavaObject),
    Enum(Handle, Handle), // classDesc, constantName
}

impl Reference {
    fn as_string(&self) -> ReadResult<&str> {
        match self {
            Self::JavaString(text) => Ok(text),
            _ => Err(StreamError::InvalidReference("String")),
        }
    }
    fn class_outline(&self) -> ReadResult<&ClassOutline> {
        match self {
            Self::Class(outline) => Ok(outline),
            _ => Err(StreamError::InvalidReference("ClassOutline")),
        }
    }
}

#[derive(Debug, Clone)]
struct ClassOutline {
    class_name: String,
    _serial_uid: u64,
    super_class: Option<Handle>,
    fields: Vec<FieldSpec>,
    _annotations: Vec<Annotation>,
    flags: Flag,
}

#[derive(Debug)]
struct ProxyOutline {
    _interfaces: Vec<String>,
    _annotations: Vec<Annotation>,
    class_outline: Handle, //  -> Class(outline)
}

#[derive(Debug, Clone)]
struct FieldSpec {
    name: String,
    type_spec: char,
}

#[derive(Debug)]
enum ReadOrder {
    Fields(Vec<FieldSpec>),
    Annotations,
}

/// Java's primitive value types in Rust form
///
/// The boxed versions of primitives (`java.lang.Long` etc) are resolved as
/// objects and are not returned as primitives.
#[derive(Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PrimitiveType {
    /// byte as u8
    Byte(u8),
    /// char as char
    ///
    /// A Java char is always 2 bytes whereas a Rust char is four but for now
    /// even with the increased memory usage, using a char presents a better
    /// API than storing it as a u16.
    Char(char),
    /// double as f64
    Double(f64),
    /// float as f32
    Float(f32),
    /// int as i32
    Int(i32),
    /// long as i64
    Long(i64),
    /// short as i16
    Short(i16),
    /// boolean as bool
    Boolean(bool),
}

impl Display for PrimitiveType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Self::Byte(b) => write!(f, "{b:X}")?,
            Self::Char(c) => write!(f, "{c}")?,
            Self::Double(d) => write!(f, "{d}")?,
            Self::Float(fl) => write!(f, "{fl}")?,
            Self::Int(i) => write!(f, "{i}")?,
            Self::Short(s) => write!(f, "{s}")?,
            Self::Long(l) => write!(f, "{l}")?,
            Self::Boolean(b) => write!(f, "{b}")?,
        }
        Ok(())
    }
}

#[derive(Debug)]
struct JavaObject {
    class: Handle,
    fields: HashMap<String, Field>,
    annotations: Vec<Vec<Annotation>>, // vec of annotations for each class in hierarchy
}

#[derive(Debug, Clone)]
enum Annotation {
    Ref(Handle),
    Block(Vec<u8>),
}

#[derive(Debug)]
enum Field {
    Primitive(PrimitiveType),
    Reference(Handle),
    Loop(Handle),
}

/// Object data representing serialized Java object
///
/// Gives access to field data and class as well as any raw data added via
/// a custom writeObject/writeExternal method.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ObjectData {
    class: String,
    fields: HashMap<String, Value>,
    annotations: Vec<Vec<Content>>, // list of annotations for each class in the hierarchy
}

impl ObjectData {
    /// Get the fully qualified class name of the type of this object
    pub fn class_name(&self) -> &str {
        &self.class
    }
    /// Get the value associated with a field if it exists
    ///
    /// `None` indicates that the field is not present. A `null` value will
    /// be returned as `Some(Value::Null)`.
    pub fn get_field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    /// Get field and convert to Rust entity
    pub fn get_field_as<T: FromJava>(&self, name: &str) -> ConversionResult<T> {
        match self.get_field(name) {
            Some(v) => Ok(T::from_value(v)?),
            None => Err(ConversionError::FieldNotFound(name.to_string())),
        }
    }

    /// Get the annotations written by a class in this object's class hierachy
    ///
    /// eg if `Child` extends `Parent`, then `get_annotation(0)` on an instance of `Child` will return
    /// the annotations written by `Parent` and `get_annotation(1)` will return annotations written
    /// by `Child`.
    pub fn get_annotation(&self, ind: usize) -> Option<annotations::AnnotationIter> {
        self.annotations
            .get(ind)
            .map(|anno| annotations::AnnotationIter::new(anno))
    }
    /// Get the total number of object annotations added to this object by any of the classes
    /// in its class hierarchy.
    pub fn annotation_count(&self) -> usize {
        self.annotations.iter().filter(|a| !a.is_empty()).count()
    }
    /// Get the number of fields written for this object
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
}

/// The possible values written by Java's serialization
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Value {
    /// A Java null reference
    Null,
    /// A 'normal' Java Object
    Object(ObjectData),
    /// A String. These are treated differently to normal objects.
    JavaString(String),
    /// An instance of an Enum. Only the class name and variant name are available
    Enum(String, String), // class name, constant name
    /// A Java primitive - int, long, double etc
    Primitive(PrimitiveType),
    /// An array of Java Objects
    Array(Vec<Value>),
    /// An array of Java Primitives
    PrimitiveArray(Vec<PrimitiveType>),
    /// A class object eg java.lang.String. Only the name is recorded
    Class(String),
    /// A recursive reference to something containing this value
    /// The contained value is the number of steps out to read the target
    Loop(i32),
}

impl Value {
    /// Get the primitive value this Value represents
    /// # Panics
    /// If this value is not a primitive
    pub fn primitive(&self) -> &PrimitiveType {
        match self {
            Value::Primitive(pt) => pt,
            _ => panic!("Can't get primitive type from non primitive value"),
        }
    }
    /// Get the array of values this Value represents
    /// # Panics
    /// If this value is not an array. Note, this method expects an array of
    /// objects and a primitive array will also panic. See primitive_array()
    pub fn array(&self) -> &[Value] {
        match self {
            Value::Array(values) => values,
            _ => panic!("Can't get array values from non-array"),
        }
    }
    /// Get the array of primitive this Value represents
    /// # Panics
    /// If this value is not an array of primitives
    pub fn primitive_array(&self) -> &[PrimitiveType] {
        match self {
            Value::PrimitiveArray(values) => values,
            _ => panic!("Can't get array values from non-array"),
        }
    }
    /// Check if this value is a null reference
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    /// Get the string value the value represents. This is only used to get
    /// the string from a deserialised Java String and will not convert other
    /// types into strings.
    /// # Panics
    /// If this value is not a JavaString
    pub fn string(&self) -> &str {
        match self {
            Value::JavaString(s) => s,
            _ => panic!("Not a string value"),
        }
    }
    /// Get the object data of the object this value represents.
    /// # Panics
    /// If this value is not an instance of an object
    pub fn object_data(&self) -> &ObjectData {
        match self {
            Value::Object(obj) => obj,
            _ => panic!("Not an object"),
        }
    }
    /// Get the class name and variant name of the enum this value represents
    /// # Panics
    /// If this value is not an enum
    pub fn enum_data(&self) -> (&str, &str) {
        match self {
            Value::Enum(cls, cons) => (cls, cons),
            _ => panic!("Not an enum"),
        }
    }
}

/// The content read from a stream.
///
/// Either an object or raw primitive data.
/// Also used to represent class and object annotations for custom write methods
///
/// This is the top level type that clients will interact with. When Java code
/// writes to a stream it can choose to write
///  * objects, which will each be read into an instance of Object (containing its data)
///  * primitives, which are written as bytes and returned as instances of
///    Block (containing the data).
///
///    The individual java primitives are not distinguished in the stream
///    so that writing `(short)1` twice and writing `(int)65537` once result in
///    the same stream being wrtten. For this reason, decoding the bytes read is
///    left to the client code that hopefully knows what format to expect.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Content {
    /// A deserialized Java Object
    Object(Value),
    /// An array of raw data
    Block(Vec<u8>),
}

impl Content {
    /// Get the value of the object represented by this instance
    ///
    /// panics if this Content is raw block data
    pub fn value(&self) -> &Value {
        match self {
            Content::Object(val) => val,
            _ => panic!("Can't unwrap block data to value"),
        }
    }
    /// Get the raw block data represented by this instance
    ///
    /// panics is this Content is an object value
    pub fn data(&self) -> &[u8] {
        match self {
            Content::Block(data) => data,
            _ => panic!("Can't unwrap value as block data"),
        }
    }
}

#[cfg(test)]
mod parser_tests {
    use super::StreamRecord::*;
    use super::*;
    #[test]
    fn invalid_stream() {
        let dat: &[u8] = &[0xAC, 0xDE, 0x00, 0x05];
        let parser = Parser::new(dat);
        match parser {
            Ok(_) => panic!("Parser shouldn't be created with invalid magic marker"),
            Err(JavaError::ReadError(StreamError::NonJavaObject(mm))) => assert_eq!(
                mm, 0xACDE,
                "NonJavaObject error has wrong magic marker data",
            ),
            Err(e) => panic!("Error should have been NonJavaObject but was '{}'", e),
        }
    }
    #[test]
    fn invalid_stream_version() {
        let dat: &[u8] = &[0xAC, 0xED, 0x00, 0x06];
        let parser = Parser::new(dat);
        match parser {
            Ok(_) => panic!("Parser shouldn't be created with unknown version"),
            Err(JavaError::ReadError(StreamError::UnknownVersion(version))) => {
                assert_eq!(version, 6, "UnknownVersion error has wrong version",)
            }
            Err(e) => panic!("Error should have been UnknownVersion but was '{}'", e),
        }
    }
    #[test]
    fn correct_marker_and_version() {
        let dat: &[u8] = &[0xAC, 0xED, 0x00, 0x05];
        let parser = Parser::new(dat);
        let parser = parser.expect("Parser should be created with valid stream");
        assert_eq!(parser._version, 5, "Parser has incorrect version");
        assert_eq!(
            parser.handle, INITIAL_HANDLE,
            "Parser started with incorrect handle"
        );
    }
    #[test]
    fn handles_are_incremented() {
        let dat: &[u8] = &[0xAC, 0xED, 0x00, 0x05];
        let mut parser = Parser::new(dat).expect("Parser failed to initialise");
        assert_eq!(parser.next_handle(), INITIAL_HANDLE);
        assert_eq!(parser.next_handle(), INITIAL_HANDLE + 1);
        assert_eq!(parser.next_handle(), INITIAL_HANDLE + 2);
    }
    #[test]
    fn handle_stack_holds_current_handles() {
        let dat: &[u8] = &[0xAC, 0xED, 0x00, 0x05];
        let mut parser = Parser::new(dat).expect("Parser failed to initialise");
        let a = parser.next_handle();
        let b = parser.next_handle();
        assert_eq!(vec![a, b], parser.handle_stack);
        parser.register(&b, Reference::Null);
        assert_eq!(vec![a], parser.handle_stack);
    }
    #[test]
    #[should_panic(expected = "object was registered before something it references")]
    fn handles_must_be_registered_in_order() {
        let dat: &[u8] = &[0xAC, 0xED, 0x00, 0x05];
        let mut parser = Parser::new(dat).expect("Parser failed to initialise");
        let a = parser.next_handle();
        let _b = parser.next_handle();
        parser.register(&a, Reference::Null);
    }
    #[test]
    fn read_null_reference() -> Result<()> {
        let dat: &[u8] = &[
            0xAC, 0xED, 0x00, 0x05, // MAGIC, VERSION
            0x70, // Null marker
        ];
        let mut parser = Parser::new(dat)?;
        let result = parser.read_from_stream()?;
        match result {
            BlockData(_) => panic!("Expected null reference but read block data"),
            EndBlockData => panic!("Expected null reference but read end block data"),
            Ref(0) => (),
            Ref(x) => panic!("Expected null reference but read Ref({}) instead", x),
        }
        assert_eq!(
            parser.handle, INITIAL_HANDLE,
            "Handle shouldn't be incremented for null"
        );
        Ok(())
    }
    #[test]
    fn short_string() -> Result<()> {
        let dat: &[u8] = &[
            0xAC, 0xED, 0x00, 0x05, // MAGIC, VERSION
            0x74, // Short java string
            0x00, 0x0A, // Length of string (10)
            0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x57, 0x6F, 0x72, 0x6C, 0x64, // string data
        ];
        let mut parser = Parser::new(dat)?;
        let result = parser.read_from_stream()?;
        match result {
            BlockData(_) => panic!("Expected string but read block data"),
            EndBlockData => panic!("Expected string but read end block data"),
            Ref(x) => {
                assert_eq!(x, INITIAL_HANDLE);
                let read = parser.get_from_handle(&x)?;
                match read {
                    Reference::JavaString(s) => assert_eq!(s, "helloWorld"),
                    x => panic!("Expected string but read {:?}", x),
                }
            }
        }
        Ok(())
    }
    #[test]
    fn repeated_string() -> Result<()> {
        let dat: &[u8] = &[
            0xAC, 0xED, 0x00, 0x05, // MAGIC, VERSION
            0x74, // Short string marker
            0x00, 0x0A, // Length of string (10)
            0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x57, 0x6F, 0x72, 0x6C, 0x64, // string data
            0x71, // back reference marker
            0x00, 0x7E, 0x00, 0x00, // initial handle given to first string
        ];
        let mut parser = Parser::new(dat)?;
        let _result = parser.read_from_stream()?;
        let second = parser.read_from_stream()?;
        match second {
            BlockData(_) => panic!("Expected string but read block data"),
            EndBlockData => panic!("Expected string but read end block data"),
            Ref(x) => {
                assert_eq!(x, INITIAL_HANDLE);
                let read = parser.get_from_handle(&x)?;
                match read {
                    Reference::JavaString(s) => assert_eq!(s, "helloWorld"),
                    x => panic!("Expected string but read {:?}", x),
                }
            }
        }
        Ok(())
    }
    #[test]
    /// This is reading a serialized class not the class description of a serialized object
    fn read_class_object() -> Result<()> {
        let dat: &[u8] = &[
            0xAC, 0xED, 0x00, 0x05, // MAGIC, VERSION
            0x76, // Class marker
            0x72, // Class description marker
            0x00, 0x11, // length of class name (17)
            0x6A, 0x61, 0x76, 0x61, 0x2E, 0x6C, 0x61, 0x6E, 0x67, 0x2E, 0x49, 0x6E, 0x74, 0x65,
            0x67, 0x65, 0x72, // string data
            0x12, 0xE2, 0xA0, 0xA4, 0xF7, 0x81, 0x87, 0x38, // SerialVersionUID
            0x02, // class desc flags (0x02 -> serializable)
            0x00, 0x01, // field count (1)
            0x49, // primitive type code ('I' -> int)
            0x00, 0x05, // length of field name
            0x76, 0x61, 0x6C, 0x75, 0x65, // field name string data
            0x78, // end block data - no class annotations
            0x72, // Class description marker for superclass
            0x00, 0x10, // length of class name (16)
            0x6A, 0x61, 0x76, 0x61, 0x2E, 0x6C, 0x61, 0x6E, 0x67, 0x2E, 0x4E, 0x75, 0x6D, 0x62,
            0x65, 0x72, // string data
            0x86, 0xAC, 0x95, 0x1D, 0x0B, 0x94, 0xE0, 0x8B, // SerialVersionUID
            0x02, // class desc flags (0x02 -> serializable)
            0x00, 0x00, // field count (0)
            0x78, // end block data - no class annotations
            0x70, // null reference - no superclass
        ];
        let mut parser = Parser::new(dat)?;
        let result = parser.read_from_stream()?;
        match result {
            BlockData(_) => panic!("Expected Class handle but read block data"),
            EndBlockData => panic!("Expected Class handle but read end block data"),
            Ref(x) => {
                assert_eq!(x, INITIAL_HANDLE + 2); // first handle given to string
                let read = parser.get_from_handle(&x)?;
                match read {
                    Reference::ClassObject(hnd) => {
                        let outline = parser.get_from_handle(hnd)?.class_outline()?;
                        assert_eq!(outline.class_name, "java.lang.Integer");
                        assert!(outline._annotations.is_empty());
                        assert_eq!(outline.flags, Flag::NoWrite);
                        let super_class =
                            parser.get_from_handle(&(outline.super_class.unwrap()))?;
                        assert_eq!(super_class.class_outline()?.class_name, "java.lang.Number");
                    }
                    x => panic!("Expected class object but read {:?}", x),
                }
            }
        }
        Ok(())
    }
    #[test]
    fn read_invalid_primitive() -> Result<()> {
        let data: &[u8] = &[0xAC, 0xED, 0x00, 0x05, 0x42];
        let mut parser = Parser::new(data)?;
        match parser.read_primitive('X') {
            Ok(p) => panic!("Expected invalid primitive, read '{}'", p),
            Err(StreamError::UnrecognisedType(e)) => assert_eq!(e, 'X'),
            Err(e) => panic!("Expect unrecognised primitive error, but got '{}'", e),
        }
        Ok(())
    }
}
