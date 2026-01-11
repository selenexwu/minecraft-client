use anyhow::anyhow;
use minecraft_derive::MinecraftData;
use std::{
    fmt::{Debug, Display},
    io::{Read, Write},
    ptr::with_exposed_provenance,
};

pub type Error = anyhow::Error;

pub trait MinecraftData: Sized + Debug {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error>;
    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error>;
    fn num_bytes(&self) -> usize;
}

#[derive(Debug, Clone, Copy)]
struct UnimplementedData;
impl MinecraftData for UnimplementedData {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        unimplemented!("decode UnimplementedData")
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        unimplemented!("encode UnimplementedData")
    }

    fn num_bytes(&self) -> usize {
        unimplemented!("num_bytes UnimplementedData")
    }
}

const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    fn num_bytes(&self) -> usize {
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
        std::fmt::Display::fmt(&self.0, f)
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

    fn num_bytes(&self) -> usize {
        VarInt(self.0.len() as i32).num_bytes() + self.0.len()
    }
}

pub type Identifier = MString<32767>;

macro_rules! impl_minecraft_data_for_num {
    ($num:ty, $bytes:expr) => {
        impl MinecraftData for $num {
            fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
                let mut buf = [0u8; $bytes];
                reader.read_exact(&mut buf)?;
                Ok(<$num>::from_be_bytes(buf))
            }

            fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
                writer.write_all(&self.to_be_bytes())?;
                Ok(())
            }

            fn num_bytes(&self) -> usize {
                $bytes
            }
        }
    };
}

macro_rules! impl_minecraft_data_for_int {
    ($int:ty) => {
        impl_minecraft_data_for_num!($int, <$int>::BITS as usize / 8);
    };
}

impl_minecraft_data_for_int!(u8);
impl_minecraft_data_for_int!(u16);
impl_minecraft_data_for_int!(u32);
impl_minecraft_data_for_int!(u64);
impl_minecraft_data_for_int!(u128);
impl_minecraft_data_for_int!(i8);
impl_minecraft_data_for_int!(i16);
impl_minecraft_data_for_int!(i32);
impl_minecraft_data_for_int!(i64);
impl_minecraft_data_for_int!(i128);
impl_minecraft_data_for_num!(f32, 4);
impl_minecraft_data_for_num!(f64, 8);

#[derive(Debug, Clone, Copy, MinecraftData)]
pub struct UUID(pub u128);

impl MinecraftData for bool {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        match buf[0] {
            0x00 => Ok(false),
            0x01 => Ok(true),
            _ => Err(anyhow!("invalid value for bool")),
        }
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        writer.write_all(&[match self {
            true => 0x01,
            false => 0x00,
        }])?;
        Ok(())
    }

    fn num_bytes(&self) -> usize {
        1
    }
}

fn decode_array<R: Read, T: MinecraftData>(len: usize, reader: &mut R) -> Result<Vec<T>, Error> {
    let mut res = Vec::with_capacity(len);
    for _ in 0..len {
        res.push(T::decode(reader)?)
    }
    Ok(res)
}

fn encode_array<W: Write, T: MinecraftData, I: IntoIterator<Item = T>>(
    data: I,
    writer: &mut W,
) -> Result<(), Error> {
    for elem in data.into_iter() {
        elem.encode(writer)?;
    }
    Ok(())
}

fn num_bytes_array<'a, T: MinecraftData + 'a, I: IntoIterator<Item = &'a T>>(data: I) -> usize {
    data.into_iter()
        .map(MinecraftData::num_bytes)
        .sum::<usize>()
}

impl<T: MinecraftData, const N: usize> MinecraftData for [T; N] {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        // cannot fail bc we know we put the right number of elements in
        Ok(decode_array(N, reader)?.try_into().unwrap())
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        encode_array(self, writer)
    }

    fn num_bytes(&self) -> usize {
        num_bytes_array(self)
    }
}

impl<T: MinecraftData> MinecraftData for Vec<T> {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let len = VarInt::decode(reader)?.0 as usize;
        decode_array(len, reader)
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        VarInt(self.len() as i32).encode(writer)?;
        encode_array(self, writer)
    }

    fn num_bytes(&self) -> usize {
        VarInt(self.len() as i32).num_bytes() + num_bytes_array(self)
    }
}

impl<T: MinecraftData> MinecraftData for Option<T> {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let is_present = bool::decode(reader)?;
        if is_present {
            Ok(Some(T::decode(reader)?))
        } else {
            Ok(None)
        }
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        match self {
            Some(val) => {
                true.encode(writer)?;
                val.encode(writer)
            }
            None => false.encode(writer),
        }
    }

    fn num_bytes(&self) -> usize {
        match self {
            Some(val) => 1 + val.num_bytes(),
            None => 1,
        }
    }
}

#[derive(Debug, Clone, MinecraftData)]
pub struct GameProfileProperty {
    pub name: MString<64>,
    pub value: MString<32767>,
    pub signature: Option<MString<1024>>,
}

#[derive(Debug, Clone, MinecraftData)]
pub struct GameProfile {
    pub uuid: UUID,
    pub username: MString<16>,
    pub properties: Vec<GameProfileProperty>,
}

#[derive(Debug, Clone, MinecraftData)]
pub struct Tag {
    name: Identifier,
    entries: Vec<VarInt>,
}

#[derive(Debug, Clone, Copy, MinecraftData)]
pub struct Position(i64);

impl Position {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Position(
            ((x as i64 & 0x3FFFFFF) << 38) | ((z as i64 & 0x3FFFFFF) << 12) | (y as i64 & 0xFFF),
        )
    }

    pub fn x(&self) -> i32 {
        (self.0 >> 38) as i32
    }
    pub fn y(&self) -> i32 {
        (self.0 << 52 >> 52) as i32
    }
    pub fn z(&self) -> i32 {
        (self.0 << 26 >> 38) as i32
    }
}

#[derive(Debug, Clone)]
pub enum IDSet {
    Named(Identifier),
    Enumerated(Vec<VarInt>),
}

impl MinecraftData for IDSet {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let len = VarInt::decode(reader)?.0 as usize;
        if len == 0 {
            Ok(Self::Named(Identifier::decode(reader)?))
        } else {
            Ok(Self::Enumerated(decode_array(len - 1, reader)?))
        }
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        match self {
            Self::Named(tag) => {
                VarInt(0).encode(writer)?;
                tag.encode(writer)?;
            }
            Self::Enumerated(ids) => {
                VarInt(ids.len() as i32 + 1).encode(writer)?;
                encode_array(ids, writer)?;
            }
        }
        Ok(())
    }

    fn num_bytes(&self) -> usize {
        match self {
            Self::Named(tag) => VarInt(0).num_bytes() + tag.num_bytes(),
            Self::Enumerated(ids) => {
                VarInt(ids.len() as i32 + 1).num_bytes() + num_bytes_array(ids)
            }
        }
    }
}

#[derive(Debug, Clone, MinecraftData)]
pub struct Slot {
    count: VarInt,
    #[present_if(count.0 > 0)]
    id: Option<VarInt>,
    #[present_if(count.0 > 0)]
    num_components_add: Option<VarInt>,
    #[present_if(count.0 > 0)]
    num_components_remove: Option<VarInt>,
    #[present_if(num_components_add.is_some_and(|x| x.0 > 0))]
    components_add: Option<UnimplementedData>,
    #[present_if(num_components_remove.is_some_and(|x| x.0 > 0))]
    components_remove: Option<UnimplementedData>,
}

#[derive(Debug, Clone)]
pub enum SlotDisplay {
    Empty,
    AnyFuel,
    Item {
        item_type: VarInt,
    },
    ItemStack {
        item_stack: Slot,
    },
    Tag {
        tag: Identifier,
    },
    SmithingTrim {
        base: Box<SlotDisplay>,
        material: Box<SlotDisplay>,
        pattern: VarInt,
    },
    WithRemainder {
        ingredient: Box<SlotDisplay>,
        remainder: Box<SlotDisplay>,
    },
    Composite {
        options: Vec<SlotDisplay>,
    },
}

// TODO: this should be macroable
impl MinecraftData for SlotDisplay {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        match VarInt::decode(reader)? {
            VarInt(0) => Ok(Self::Empty),
            VarInt(1) => Ok(Self::AnyFuel),
            VarInt(2) => Ok(Self::Item {
                item_type: VarInt::decode(reader)?,
            }),
            VarInt(3) => Ok(Self::ItemStack {
                item_stack: Slot::decode(reader)?,
            }),
            VarInt(4) => Ok(Self::Tag {
                tag: Identifier::decode(reader)?,
            }),
            VarInt(5) => Ok(Self::SmithingTrim {
                base: Box::new(SlotDisplay::decode(reader)?),
                material: Box::new(SlotDisplay::decode(reader)?),
                pattern: VarInt::decode(reader)?,
            }),
            VarInt(6) => Ok(Self::WithRemainder {
                ingredient: Box::new(SlotDisplay::decode(reader)?),
                remainder: Box::new(SlotDisplay::decode(reader)?),
            }),
            VarInt(7) => Ok(Self::Composite {
                options: Vec::decode(reader)?,
            }),
            _ => Err(anyhow!("Invalid SlotDisplay")),
        }
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        match self {
            SlotDisplay::Empty => VarInt(0).encode(writer),
            SlotDisplay::AnyFuel => VarInt(1).encode(writer),
            SlotDisplay::Item { item_type } => {
                VarInt(2).encode(writer)?;
                item_type.encode(writer)
            }
            SlotDisplay::ItemStack { item_stack } => {
                VarInt(3).encode(writer)?;
                item_stack.encode(writer)
            }
            SlotDisplay::Tag { tag } => {
                VarInt(4).encode(writer)?;
                tag.encode(writer)
            }
            SlotDisplay::SmithingTrim {
                base,
                material,
                pattern,
            } => {
                VarInt(5).encode(writer)?;
                base.encode(writer)?;
                material.encode(writer)?;
                pattern.encode(writer)
            }
            SlotDisplay::WithRemainder {
                ingredient,
                remainder,
            } => {
                VarInt(6).encode(writer)?;
                ingredient.encode(writer)?;
                remainder.encode(writer)
            }
            SlotDisplay::Composite { options } => {
                VarInt(7).encode(writer)?;
                options.encode(writer)
            }
        }
    }

    fn num_bytes(&self) -> usize {
        match self {
            SlotDisplay::Empty => VarInt(0).num_bytes(),
            SlotDisplay::AnyFuel => VarInt(1).num_bytes(),
            SlotDisplay::Item { item_type } => VarInt(2).num_bytes() + item_type.num_bytes(),
            SlotDisplay::ItemStack { item_stack } => VarInt(3).num_bytes() + item_stack.num_bytes(),
            SlotDisplay::Tag { tag } => VarInt(4).num_bytes() + tag.num_bytes(),
            SlotDisplay::SmithingTrim {
                base,
                material,
                pattern,
            } => {
                VarInt(5).num_bytes()
                    + base.num_bytes()
                    + material.num_bytes()
                    + pattern.num_bytes()
            }
            SlotDisplay::WithRemainder {
                ingredient,
                remainder,
            } => VarInt(6).num_bytes() + ingredient.num_bytes() + remainder.num_bytes(),
            SlotDisplay::Composite { options } => VarInt(7).num_bytes() + options.num_bytes(),
        }
    }
}
