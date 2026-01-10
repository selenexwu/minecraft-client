use std::{
    io::{BufReader, BufWriter, Read, Write},
    net::TcpStream,
};

use anyhow::Result;

use crate::{
    datatypes::{MinecraftData, VarInt, UUID},
    packet::{
        decode_packet_header, AcknowledgeFinishConfigurationPacket,
        ClientboundConfigurationPluginMessagePacket, ClientboundKnownPacksPacket,
        ClientboundPlayKeepAlivePacket, ConfigurationKeepAlivePacket,
        ConfigurationUpdateTagsPacket, FeatureFlagsPacket, FinishConfigurationPacket,
        HandshakeIntent, HandshakePacket, LoginAcknowledgedPacket, LoginStartPacket,
        LoginSuccessPacket, Packet, PacketHeader, RegistryDataPacket, ServerboundKnownPacksPacket,
        ServerboundPlayKeepAlivePacket, StatusRequestPacket, StatusResponsePacket,
    },
};

const DEBUG_SENT_PACKETS: bool = false;

pub struct Connection {
    host: String,
    port: u16,
    writer: BufWriter<TcpStream>,
    reader: BufReader<TcpStream>,
}

impl Connection {
    pub fn connect(host: String, port: u16) -> Result<Connection> {
        let stream = TcpStream::connect((host.as_str(), port))?;
        Ok(Connection {
            host,
            port,
            writer: BufWriter::new(stream.try_clone()?),
            reader: BufReader::new(stream),
        })
    }

    fn send_packet<P: Packet>(&mut self, packet: P) -> Result<()> {
        if DEBUG_SENT_PACKETS {
            let mut bytes = Vec::new();
            packet.encode_packet(&mut bytes)?;
            eprintln!("{:?}", bytes);
            self.writer.write_all(&bytes)?;
        } else {
            packet.encode_packet(&mut self.writer)?;
        }
        self.writer.flush()?;
        Ok(())
    }

    fn recv_packet_header(&mut self) -> Result<PacketHeader> {
        decode_packet_header(&mut self.reader)
    }

    fn recv_packet<P: Packet>(&mut self) -> Result<P> {
        P::decode_packet(&mut self.reader)
    }

    fn recv_packet_raw(&mut self, header: &PacketHeader) -> Result<Vec<u8>> {
        let mut res = vec![0u8; header.len.0 as usize - header.id.len()];
        self.reader.read_exact(&mut res)?;
        Ok(res)
    }

    /// Takes self because this closes the connection
    pub fn get_status(mut self) -> Result<String> {
        self.send_packet(HandshakePacket {
            protocol_version: VarInt(-1),
            server_address: self.host.clone().try_into()?,
            server_port: self.port,
            intent: HandshakeIntent::Status,
        })?;
        self.send_packet(StatusRequestPacket)?;

        // TODO: maybe read first and then read exactly enough bytes?
        //       or at least validate this explicitly
        let _resp_header = self.recv_packet_header()?;
        // eprintln!("{resp_header:?}");
        let resp = self.recv_packet::<StatusResponsePacket>()?;
        // eprintln!("{resp:?}");
        Ok(resp.json_response.to_string())
    }

    pub fn login(&mut self) -> Result<()> {
        self.send_packet(HandshakePacket {
            protocol_version: VarInt(773),
            server_address: self.host.clone().try_into()?,
            server_port: self.port,
            intent: HandshakeIntent::Login,
        })?;
        self.send_packet(LoginStartPacket {
            name: "robotabc773".to_string().try_into()?,
            // uuid: UUID(0xcf766be42bed41bdb40ae0c22ac798f1),
            uuid: UUID(0),
        })?;

        // TODO: enable online mode and use authentication and encryption
        // let resp_header = self.recv_packet_header()?;
        // eprintln!("{:?}", resp_header);
        // let resp = self.recv_packet::<EncryptionRequestPacket>()?;
        // eprintln!("{:?}", resp);

        // let key: RsaPublicKey =
        //     SubjectPublicKeyInfoRef::try_from(resp.public_key.as_slice())?.try_into()?;
        // eprintln!("{:?}", key);

        // TODO: enable compression

        let resp_header = self.recv_packet_header()?;
        eprintln!("{:?}", resp_header);
        let resp = self.recv_packet::<LoginSuccessPacket>()?;
        eprintln!("{:?}", resp);
        self.send_packet(LoginAcknowledgedPacket)?;

        Ok(())
    }

    pub fn configure(&mut self) -> Result<()> {
        loop {
            // TODO: macro for handling packets
            let resp_header = self.recv_packet_header()?;
            eprintln!("{:?}", resp_header);
            match resp_header.id {
                val if val == ClientboundConfigurationPluginMessagePacket::ID => {
                    let resp = self.recv_packet::<ClientboundConfigurationPluginMessagePacket>()?;
                    eprintln!("{:?}", resp);
                }
                val if val == FeatureFlagsPacket::ID => {
                    let resp = self.recv_packet::<FeatureFlagsPacket>()?;
                    eprintln!("{:?}", resp);
                }
                val if val == ClientboundKnownPacksPacket::ID => {
                    let resp = self.recv_packet::<ClientboundKnownPacksPacket>()?;
                    eprintln!("{:?}", resp);
                    self.send_packet(ServerboundKnownPacksPacket {
                        known_packs: resp.known_packs,
                    })?;
                }
                val if val == ConfigurationKeepAlivePacket::ID => {
                    let resp = self.recv_packet::<ConfigurationKeepAlivePacket>()?;
                    eprintln!("{:?}", resp);
                    self.send_packet(ConfigurationKeepAlivePacket {
                        keep_alive_id: resp.keep_alive_id,
                    })?;
                }
                val if val == RegistryDataPacket::ID => {
                    // let resp = self.recv_packet::<RegistryDataPacket>()?;
                    let resp = self.recv_packet_raw(&resp_header)?;
                    // eprintln!("{:?}", resp);
                }
                val if val == ConfigurationUpdateTagsPacket::ID => {
                    let resp = self.recv_packet::<ConfigurationUpdateTagsPacket>()?;
                    // eprintln!("{:?}", resp);
                }
                val if val == FinishConfigurationPacket::ID => {
                    let resp = self.recv_packet::<FinishConfigurationPacket>()?;
                    eprintln!("{:?}", resp);
                    self.send_packet(AcknowledgeFinishConfigurationPacket)?;
                    break;
                }
                _ => {
                    let resp = self.recv_packet_raw(&resp_header)?;
                    eprintln!("{:?}", resp);
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn play(&mut self) -> Result<()> {
        loop {
            let resp_header = self.recv_packet_header()?;
            eprintln!("{:?}", resp_header);
            match resp_header.id {
                val if val == ClientboundPlayKeepAlivePacket::ID => {
                    let resp = self.recv_packet::<ClientboundPlayKeepAlivePacket>()?;
                    eprintln!("{:?}", resp);
                    self.send_packet(ServerboundPlayKeepAlivePacket {
                        keep_alive_id: resp.keep_alive_id,
                    })?;
                }
                _ => {
                    let resp = self.recv_packet_raw(&resp_header)?;
                    // eprintln!("{:?}", resp);
                    // break;
                }
            }
        }

        Ok(())
    }
}
