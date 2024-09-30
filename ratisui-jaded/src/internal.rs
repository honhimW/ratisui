use crate::{ReadResult, StreamError};
use std::convert::TryFrom;
use std::io::Read;

const WRITE: u8 = 0x01;
const SERIALIZABLE: u8 = 0x02;
const EXTERNALIZABLE: u8 = 0x04;
const BLOCK: u8 = 0x08;
// const ENUM: u8 = 0x10;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Flag {
    NoWrite,
    Write,
    ExtBlock,
    Ext,
}

impl TryFrom<u8> for Flag {
    type Error = crate::StreamError;
    fn try_from(byte: u8) -> ReadResult<Self> {
        if byte.is(SERIALIZABLE) && !byte.is(WRITE) {
            Ok(Self::NoWrite)
        } else if byte.is(SERIALIZABLE) && byte.is(WRITE) {
            Ok(Self::Write)
        } else if byte.is(EXTERNALIZABLE) && !byte.is(BLOCK) {
            Ok(Self::Ext)
        } else if byte.is(EXTERNALIZABLE) && byte.is(BLOCK) {
            Ok(Self::ExtBlock)
        } else {
            Err(StreamError::InvalidStream("Unexpected class flag"))
        }
    }
}

trait BitField {
    fn is(&self, mask: Self) -> bool;
}

impl BitField for u8 {
    fn is(&self, other: u8) -> bool {
        self & other > 0
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum Marker {
    Null,           // nullReference
    Reference,      // prevObject
    ClassDesc,      // newClassDesc
    Object,         // newObject
    JavaString,     // newString
    Array,          // newArray
    Class,          // newClass
    BlockData,      // blockdatashort
    EndBlockData,   // endblockdata
    Reset,          // reset
    BlockDataLong,  // blockdatalong
    Exception,      // exception
    LongString,     // newString
    ProxyClassDesc, // proxyClassDesc
    Enum,           // newEnum
}

impl Marker {
    pub(crate) fn from(mark: u8) -> ReadResult<Self> {
        Ok(match mark {
            0x70 => Self::Null,           // nullReference
            0x71 => Self::Reference,      // prevObject
            0x72 => Self::ClassDesc,      // newClassDesc
            0x73 => Self::Object,         // newObject
            0x74 => Self::JavaString,     // newString
            0x75 => Self::Array,          // newArray
            0x76 => Self::Class,          // newClass
            0x77 => Self::BlockData,      // blockdatashort
            0x78 => Self::EndBlockData,   // endblockdata
            0x79 => Self::Reset,          // reset
            0x7A => Self::BlockDataLong,  // blockdatalong
            0x7B => Self::Exception,      // exception
            0x7C => Self::LongString,     // newString
            0x7D => Self::ProxyClassDesc, // proxyClassDesc
            0x7E => Self::Enum,           // newEnum
            unk => return Err(StreamError::UnknownMark(unk)),
        })
    }
}

pub(crate) trait JavaStream {
    fn read_u8(&mut self) -> ReadResult<u8>;
    fn read_u16(&mut self) -> ReadResult<u16>;
    fn read_u32(&mut self) -> ReadResult<u32>;
    fn read_u64(&mut self) -> ReadResult<u64>;
    fn read_string(&mut self) -> ReadResult<String>;
    fn read_long_string(&mut self) -> ReadResult<String>;
    fn read_marker(&mut self) -> ReadResult<Marker>;
}

impl<T: Read> JavaStream for T {
    fn read_u8(&mut self) -> ReadResult<u8> {
        let mut buffer = [0];
        self.read_exact(&mut buffer)?;
        Ok(u8::from_be_bytes(buffer))
    }
    fn read_u16(&mut self) -> ReadResult<u16> {
        let mut buffer = [0; 2];
        self.read_exact(&mut buffer)?;
        Ok(u16::from_be_bytes(buffer))
    }
    fn read_u32(&mut self) -> ReadResult<u32> {
        let mut buffer = [0; 4];
        self.read_exact(&mut buffer)?;
        Ok(u32::from_be_bytes(buffer))
    }
    fn read_u64(&mut self) -> ReadResult<u64> {
        let mut buffer = [0; 8];
        self.read_exact(&mut buffer)?;
        Ok(u64::from_be_bytes(buffer))
    }
    fn read_string(&mut self) -> ReadResult<String> {
        let len = self.read_u16()?.into();
        read_string(self, len)
    }
    fn read_long_string(&mut self) -> ReadResult<String> {
        let len = self.read_u64()? as usize;
        read_string(self, len)
    }
    fn read_marker(&mut self) -> ReadResult<Marker> {
        Marker::from(self.read_u8()?)
    }
}

fn read_string(stream: &mut dyn Read, len: usize) -> ReadResult<String> {
    let mut buffer = Vec::with_capacity(len);
    let read = stream.take(len as u64).read_to_end(&mut buffer)?;
    if read != len {
        return Err(StreamError::InvalidStream("Could not read full string"));
    }
    Ok(String::from_utf8(buffer)?)
}

#[cfg(test)]
mod java_stream_test {
    use super::*;
    const DEMO_STREAM: &[u8] = &[
        0x00, 0x0A, 0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x57, 0x6F, 0x72, 0x6C, 0x64, // string data
    ];
    fn demo_stream() -> impl Read {
        DEMO_STREAM
    }
    #[test]
    fn read_u8() {
        assert_eq!(demo_stream().read_u8().expect("Failed to read u8"), 0);
    }
    #[test]
    fn read_u16() {
        assert_eq!(demo_stream().read_u16().expect("Failed to read u16"), 10);
    }
    #[test]
    fn read_u32() {
        assert_eq!(
            demo_stream().read_u32().expect("Failed to read u32"),
            682_085
        );
    }
    #[test]
    fn read_u64() {
        assert_eq!(
            demo_stream().read_u64().expect("Failed to read u64"),
            2_929_534_587_137_879
        );
    }
    #[test]
    fn read_string() {
        assert_eq!(
            demo_stream().read_string().expect("Failed to read string"),
            "helloWorld"
        );
    }
    #[test]
    fn read_markers() -> ReadResult<()> {
        let mut data: &[u8] = &[0x74];
        assert_eq!(data.read_marker()?, Marker::JavaString);
        Ok(())
    }
    #[test]
    fn read_incomplete_string() {
        let mut data: &[u8] = &[
            0x00, 0x10, 0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x57, // partial string data
        ];
        let result = data.read_string();
        match result {
            Ok(s) => panic!("Expected string read to fail but read '{}'", s),
            Err(StreamError::InvalidStream(x)) => assert_eq!(x, "Could not read full string"),
            Err(e) => panic!("Expected InvalidStream error but got '{}'", e),
        }
    }
    #[test]
    fn read_invalid_utf8() {
        let mut data: &[u8] = &[
            0x00, 0x0A, // 10 characters
            0xff, 0xfd, 0xff, 0xfd, 0xff, 0xfd, 0xff, 0xfd, 0xff, 0xfd, 0xff,
            0xfd, // invalid data
        ];
        let result = data.read_string();
        match result {
            Ok(s) => panic!("Expected invalid utf-8 error but read '{}'", s),
            Err(StreamError::InvalidStream(e)) => assert_eq!(e, "String is not valid UTF-8"),
            Err(e) => panic!("Expected invalid stream error but got '{}'", e),
        }
    }
}

#[cfg(test)]
mod flag_test {
    use super::*;
    #[test]
    fn converting_flags() -> ReadResult<()> {
        assert_eq!(Flag::NoWrite, Flag::try_from(2)?);
        assert_eq!(Flag::Write, Flag::try_from(3)?);
        assert_eq!(Flag::Ext, Flag::try_from(4)?);
        assert_eq!(Flag::ExtBlock, Flag::try_from(12)?);

        let result = Flag::try_from(8);
        match result {
            Ok(f) => panic!("Expected error but got {:?}", f),
            Err(StreamError::InvalidStream(e)) => assert_eq!(e, "Unexpected class flag"),
            Err(e) => panic!("Expected invalid stream error but got {}", e),
        }
        Ok(())
    }
}
