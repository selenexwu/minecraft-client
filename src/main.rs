use std::io::{BufReader, BufWriter, Write};
use std::net::TcpStream;

use anyhow::Result;
use minecraft_client::datatypes::{MinecraftData, VarInt};
use minecraft_client::packet::{
    decode_packet_header, HandshakeIntent, HandshakePacket, Packet, StatusRequestPacket,
    StatusResponsePacket,
};

fn connect(host: &str, port: u16) -> Result<TcpStream> {
    Ok(TcpStream::connect((host, port))?)
}

fn main() -> Result<()> {
    let host = "localhost";
    // let host = "play.budpe.com";
    let port = 25565;
    let conn = connect(&host, port)?;
    let mut writer = BufWriter::new(&conn);
    let mut reader = BufReader::new(&conn);

    let handshake_packet = HandshakePacket::new(
        VarInt(-1),
        host.to_string().try_into()?,
        port,
        HandshakeIntent::Status,
    );
    let status_req_packet = StatusRequestPacket();
    handshake_packet.encode_packet(&mut writer)?;
    status_req_packet.encode_packet(&mut writer)?;
    writer.flush()?;

    // TODO: maybe read first and then read exactly enough bytes?
    //       or at least validate this explicitly
    let resp_header = decode_packet_header(&mut reader);
    eprintln!("{resp_header:?}");
    let resp = StatusResponsePacket::decode(&mut reader)?;
    eprintln!("{resp:?}");

    println!("{}", resp.json_response);

    Ok(())
}
