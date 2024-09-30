use super::{Content, ConversionError, ConversionResult, FromJava, Value};
use std::convert::TryInto;

enum AnnotationReadState<'a> {
    Complete,
    Value(&'a Value),
    Block(&'a [u8]),
}

impl<'a> AnnotationReadState<'a> {
    fn switch(&mut self, next: &'a Content) {
        *self = match next {
            Content::Block(d) => Self::Block(&d[..]),
            Content::Object(v) => Self::Value(v),
        }
    }
}

/// Utility for reading things from annotations
///
/// This is intended to offer a similar interface to the ObjectInputStream used
/// by Java classes that implement custom readObject methods.
pub struct AnnotationIter<'a> {
    values: Vec<&'a Content>,
    state: AnnotationReadState<'a>,
}

impl<'a> AnnotationIter<'a> {
    pub(crate) fn new(values: &'a [Content]) -> Self {
        use AnnotationReadState::*;
        let state = match values.first() {
            Some(Content::Block(d)) => Block(&d[..]),
            Some(Content::Object(v)) => Value(v),
            None => Complete,
        };
        AnnotationIter {
            values: values.iter().skip(1).collect::<Vec<_>>(),
            state,
        }
    }

    fn advance(&mut self) {
        use AnnotationReadState::*;
        match (self.values.len(), &self.state) {
            (0, Complete) => (), // End of iteration - nothing to do
            (0, Block(d)) if d.is_empty() => self.state = Complete,
            (0, Value(_)) => self.state = Complete,
            (_, Block(d)) if d.is_empty() => self.state.switch(self.values.remove(0)),
            (_, Value(_)) => self.state.switch(self.values.remove(0)),
            (_, Block(_)) => (), // mid block - nothing to do
            (_, Complete) => panic!("Annotations complete while contents remain"),
        }
    }

    fn read_bytes<T>(&mut self, count: usize) -> ConversionResult<T>
    where
        T: std::convert::TryFrom<&'a [u8]>,
    {
        self.advance();
        use AnnotationReadState::*;
        match self.state {
            Block(d) if d.len() >= count => {
                let (l, r) = d.split_at(count);
                self.state = Block(r);
                Ok(l.try_into()
                    .map_err(|_| ConversionError::InvalidType("integer data"))?)
            }
            Block(_) => Err(ConversionError::InvalidType("Not enough data")),
            Value(_) => Err(ConversionError::InvalidType("Expected block data")),
            Complete => Err(ConversionError::InvalidType("End of Annotations")),
        }
    }

    /// Read byte from annotation
    /// # Errors
    /// [ConversionError::InvalidType]
    /// * if there is not enough data
    /// * if the next item in the annotation is an
    /// object instead of binary data.
    /// # See
    /// [ObjectInputStream#readByte()](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/io/ObjectInputStream.html#readByte())
    pub fn read_u8(&mut self) -> ConversionResult<u8> {
        Ok(self.read_bytes::<&[u8]>(1)?[0])
    }

    /// Read boolean from annotation
    /// # Errors
    /// [ConversionError::InvalidType]
    /// * if there is not enough data
    /// * if the next item in the annotation is an object instead of binary data.
    /// # See
    /// [ObjectInputStream#readBoolean()](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/io/ObjectInputStream.html#readBoolean())
    pub fn read_boolean(&mut self) -> ConversionResult<bool> {
        Ok(self.read_u8()? == 1)
    }

    /// Read short (i16) from annotation
    /// # Errors
    /// [ConversionError::InvalidType]
    /// * if there is not enough data
    /// * if the next item in the annotation is an object instead of binary data.
    /// # See
    /// [ObjectInputStream#readShort()](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/io/ObjectInputStream.html#readShort())
    pub fn read_i16(&mut self) -> ConversionResult<i16> {
        Ok(i16::from_be_bytes(self.read_bytes(2)?))
    }

    /// Read int (i32) from annotation
    /// # Errors
    /// [ConversionError::InvalidType]
    /// * if there is not enough data
    /// * if the next item in the annotation is an object instead of binary data.
    /// # See
    /// [ObjectInputStream#readInt()](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/io/ObjectInputStream.html#readInt())
    pub fn read_i32(&mut self) -> ConversionResult<i32> {
        Ok(i32::from_be_bytes(self.read_bytes(4)?))
    }

    /// Read long (i64) from annotation
    /// # Errors
    /// [ConversionError::InvalidType]
    /// * if there is not enough data
    /// * if the next item in the annotation is an object instead of binary data.
    /// # See
    /// [ObjectInputStream#readLong()](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/io/ObjectInputStream.html#readLong())
    pub fn read_i64(&mut self) -> ConversionResult<i64> {
        Ok(i64::from_be_bytes(self.read_bytes(8)?))
    }

    /// Read float (f32) from annotation
    /// # Errors
    /// [ConversionError::InvalidType]
    /// * if there is not enough data
    /// * if the next item in the annotation is an object instead of binary data.
    /// # See
    /// [ObjectInputStream#readFloat()](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/io/ObjectInputStream.html#readFloat())
    pub fn read_f32(&mut self) -> ConversionResult<f32> {
        Ok(f32::from_be_bytes(self.read_bytes(4)?))
    }

    /// Read double (f64) from annotation
    /// # Errors
    /// [ConversionError::InvalidType]
    /// * if there is not enough data
    /// * if the next item in the annotation is an object instead of binary data.
    /// # See
    /// [ObjectInputStream#readDouble()](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/io/ObjectInputStream.html#readDouble())
    pub fn read_f64(&mut self) -> ConversionResult<f64> {
        Ok(f64::from_be_bytes(self.read_bytes(8)?))
    }

    /// Read char from annotation
    /// # Errors
    /// [ConversionError::InvalidType]
    /// * if there is not enough data
    /// * if the next item in the annotation is an object instead of binary data.
    /// * if the bytes are not a valid UTF-8 character
    /// # See
    /// [ObjectInputStream#readChar()](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/io/ObjectInputStream.html#readChar())
    pub fn read_char(&mut self) -> ConversionResult<char> {
        let c = self.read_i16()? as u32;
        std::char::from_u32(c).ok_or(ConversionError::InvalidType("valid character"))
    }

    /// Read an object from annotation
    /// # Errors
    /// [ConversionError::UnexpectedBlockData]
    /// if the next item in annotation is binary data.
    /// [ConversionError::MissingAnnotations]
    /// if there are no more objects in this annotation
    /// # See
    /// [ObjectInputStream#readObject()](https://docs.oracle.com/en/java/javase/17/docs/api/java.base/java/io/ObjectInputStream.html#readObject())
    pub fn read_object(&mut self) -> ConversionResult<&'a Value> {
        self.advance();
        use AnnotationReadState::*;
        match self.state {
            Value(v) => Ok(v),
            Block(d) => Err(ConversionError::UnexpectedBlockData(d.to_vec())),
            Complete => Err(ConversionError::NullPointerException),
        }
    }

    /// Read an object and convert it to a rust type
    /// # Errors
    /// This method is the equivalent of T::from_value(read_object()?)
    /// so any errors raised by either method will be returned.
    pub fn read_object_as<T: FromJava>(&mut self) -> ConversionResult<T> {
        T::from_value(self.read_object()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_boolean() {
        let data = vec![Content::Block(vec![1, 0])];
        let mut a = AnnotationIter::new(&data);
        assert!(a.read_boolean().unwrap());
        assert!(!a.read_boolean().unwrap());
    }
    #[test]
    fn read_bytes() {
        let data = vec![Content::Block(vec![1, 2, 3, 4, 5])];
        let mut a = AnnotationIter::new(&data);
        assert_eq!(1, a.read_u8().unwrap());
        assert_eq!(2, a.read_u8().unwrap());
        assert_eq!(3, a.read_u8().unwrap());
        assert_eq!(4, a.read_u8().unwrap());
        assert_eq!(5, a.read_u8().unwrap());
    }

    #[test]
    fn read_u16() {
        let data = vec![Content::Block(vec![1, 2, 3, 4, 255, 5])];
        let mut a = AnnotationIter::new(&data);
        assert_eq!(258, a.read_i16().unwrap());
        assert_eq!(772, a.read_i16().unwrap());
        assert_eq!(-251, a.read_i16().unwrap());
    }

    #[test]
    fn read_i32() {
        let data = vec![Content::Block(vec![1, 2, 3, 4, 255, 255, 3, 4])];
        let mut a = AnnotationIter::new(&data);
        assert_eq!(16_909_060, a.read_i32().unwrap());
        assert_eq!(-64764, a.read_i32().unwrap());
    }

    #[test]
    fn read_i64() {
        let data = vec![Content::Block(vec![
            1, 2, 3, 4, 5, 6, 7, 8, 255, 255, 255, 255, 1, 2, 3, 4,
        ])];
        let mut a = AnnotationIter::new(&data);
        assert_eq!(72_623_859_790_382_856, a.read_i64().unwrap());
        assert_eq!(-4278058236, a.read_i64().unwrap());
    }

    #[test]
    fn read_float() {
        let data = vec![Content::Block(vec![
            0x42, 0x28, 0x00, 0x00, 0xc2, 0x28, 0x00, 0x00,
        ])];
        let mut a = AnnotationIter::new(&data);
        assert_eq!(42_f32, a.read_f32().unwrap());
        assert_eq!(-42_f32, a.read_f32().unwrap());
    }

    #[test]
    fn read_double() {
        let data = vec![Content::Block(vec![
            0x40, 0x45, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])];
        let mut a = AnnotationIter::new(&data);
        assert_eq!(42_f64, a.read_f64().unwrap());
    }

    #[test]
    fn read_char() {
        let data = vec![Content::Block(vec![
            0, 102, 0, 111, 0, 111, 0, 98, 0, 97, 0, 114,
        ])];
        let mut a = AnnotationIter::new(&data);
        assert_eq!('f', a.read_char().unwrap());
        assert_eq!('o', a.read_char().unwrap());
        assert_eq!('o', a.read_char().unwrap());
        assert_eq!('b', a.read_char().unwrap());
        assert_eq!('a', a.read_char().unwrap());
        assert_eq!('r', a.read_char().unwrap());
    }

    #[test]
    fn empty_annotation() {
        let mut a = AnnotationIter::new(&[]);
        match a.read_u8() {
            Ok(_) => panic!("Read byte from empty iterator"),
            Err(ConversionError::InvalidType(_)) => (),
            Err(_) => panic!("Incorrect error from reading byte"),
        }
    }
}
