use std::io::{Read, Write};

use anyhow::anyhow;
use minecraft_derive::MinecraftData;

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

#[derive(Debug, Clone, MinecraftData)]
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

#[derive(Debug, Clone, Copy, MinecraftData)]
pub struct StatusRequestPacket;

impl Packet for StatusRequestPacket {
    const ID: VarInt = VarInt(0x00);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct StatusResponsePacket {
    pub json_response: MString<32767>,
}

impl Packet for StatusResponsePacket {
    const ID: VarInt = VarInt(0x00);
}
