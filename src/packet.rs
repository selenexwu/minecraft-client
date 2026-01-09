use std::io::{Read, Write};

use anyhow::anyhow;

use crate::datatypes::{Error, MString, MinecraftData, VarInt};

pub trait Packet: MinecraftData {
    const ID: VarInt;

    fn encode_packet<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        let len = Self::ID.len() + self.len();
        VarInt(len as i32).encode(writer)?;
        Self::ID.encode(writer)?;
        self.encode(writer)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PacketHeader {
    pub len: VarInt,
    pub id: VarInt,
}

pub fn decode_packet_header<R: Read>(reader: &mut R) -> Result<PacketHeader, Error> {
    let len = VarInt::decode(reader)?;
    let id = VarInt::decode(reader)?;
    Ok(PacketHeader { len, id })
}

#[derive(Debug, Clone, Copy)]
pub enum HandshakeIntent {
    Status,
    Login,
    Transfer,
}

impl MinecraftData for HandshakeIntent {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let mut buf = [0u8];
        reader.read_exact(&mut buf)?;
        match buf[0] {
            0x1 => Ok(Self::Status),
            0x2 => Ok(Self::Login),
            0x3 => Ok(Self::Transfer),
            _ => Err(anyhow!("invalid handshake intent")),
        }
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        writer.write_all(&[match self {
            Self::Status => 0x1,
            Self::Login => 0x2,
            Self::Transfer => 0x3,
        }])?;
        Ok(())
    }

    fn len(&self) -> usize {
        1
    }
}

#[derive(Debug, Clone)]
pub struct HandshakePacket {
    protocol_version: VarInt,
    server_address: MString<255>,
    server_port: u16,
    intent: HandshakeIntent,
}

impl HandshakePacket {
    pub fn new(
        protocol_version: VarInt,
        server_address: MString<255>,
        server_port: u16,
        intent: HandshakeIntent,
    ) -> Self {
        HandshakePacket {
            protocol_version,
            server_address,
            server_port,
            intent,
        }
    }
}

impl Packet for HandshakePacket {
    const ID: VarInt = VarInt(0x00);
}

impl MinecraftData for HandshakePacket {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let protocol_version = VarInt::decode(reader)?;
        let server_address = MString::decode(reader)?;
        let server_port = u16::decode(reader)?;
        let intent = HandshakeIntent::decode(reader)?;
        Ok(Self {
            protocol_version,
            server_address,
            server_port,
            intent,
        })
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        self.protocol_version.encode(writer)?;
        self.server_address.encode(writer)?;
        self.server_port.encode(writer)?;
        self.intent.encode(writer)?;
        Ok(())
    }

    fn len(&self) -> usize {
        self.protocol_version.len()
            + self.server_address.len()
            + self.server_port.len()
            + self.intent.len()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StatusRequestPacket();

impl Packet for StatusRequestPacket {
    const ID: VarInt = VarInt(0x00);
}

impl MinecraftData for StatusRequestPacket {
    fn decode<R: Read>(_reader: &mut R) -> Result<Self, Error> {
        Ok(Self())
    }

    fn encode<W: Write>(self, _writer: &mut W) -> Result<(), Error> {
        Ok(())
    }

    fn len(&self) -> usize {
        0
    }
}

#[derive(Debug, Clone)]
pub struct StatusResponsePacket {
    pub json_response: MString<32767>,
}

impl Packet for StatusResponsePacket {
    const ID: VarInt = VarInt(0x00);
}

impl MinecraftData for StatusResponsePacket {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let json_response = MString::decode(reader)?;
        Ok(StatusResponsePacket { json_response })
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        self.json_response.encode(writer)?;
        Ok(())
    }

    fn len(&self) -> usize {
        self.json_response.len()
    }
}
