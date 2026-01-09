use anyhow::anyhow;
use std::{
    fmt::Display,
    io::{Read, Write},
};

pub type Error = anyhow::Error;

pub trait MinecraftData: Sized {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error>;
    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error>;
    fn len(&self) -> usize;
}

const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

#[derive(Debug, Clone, Copy)]
pub struct VarInt(pub i32);

impl MinecraftData for VarInt {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut value: i32 = 0;
        let mut position = 0;
        let mut buf = [0u8];
        while position < 32 {
            reader.read_exact(&mut buf)?;
            let curr_byte = buf[0];
            value |= ((curr_byte & SEGMENT_BITS) as i32) << position;
            if (curr_byte & CONTINUE_BIT) == 0 {
                return Ok(VarInt(value));
            }
            position += 7;
        }
        Err(anyhow!("varint too big"))
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        let mut value = self.0 as u32;
        loop {
            if (value & !(SEGMENT_BITS as u32)) == 0 {
                writer.write_all(&[value as u8])?;
                return Ok(());
            }

            writer.write_all(&[(value as u8 & SEGMENT_BITS) | CONTINUE_BIT])?;

            value >>= 7;
        }
    }

    fn len(&self) -> usize {
        if self.0 == 0 {
            return 1;
        }
        let bits = (self.0 as u32).ilog2() + 1;
        bits.div_ceil(7) as usize
    }
}

#[derive(Debug, Clone)]
pub struct MString<const N: usize>(String);

impl<const N: usize> TryFrom<String> for MString<N> {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() > N {
            return Err(anyhow!("string is too long!"));
        }
        Ok(MString(value))
    }
}

impl<const N: usize> Display for MString<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<const N: usize> MinecraftData for MString<N> {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let len = VarInt::decode(reader)?.0;
        if len < 0 {
            return Err(anyhow!("cannot have negative length string"));
        }
        let len = len as usize;
        if len > N {
            return Err(anyhow!("string is too long!"));
        }
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        Ok(MString(String::from_utf8(buf)?))
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        if self.0.len() > N {
            return Err(anyhow!("string is too long!"));
        }
        VarInt(self.0.len() as i32).encode(writer)?;
        writer.write_all(self.0.as_bytes())?;

        Ok(())
    }

    fn len(&self) -> usize {
        VarInt(self.0.len() as i32).len() + self.0.len()
    }
}

impl MinecraftData for u16 {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        writer.write_all(&self.to_be_bytes())?;
        Ok(())
    }

    fn len(&self) -> usize {
        2
    }
}
