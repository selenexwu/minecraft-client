use std::io::{Read, Write};

use anyhow::anyhow;
use minecraft_derive::MinecraftData;

use crate::datatypes::{
    Error, GameProfile, IDSet, Identifier, MString, MinecraftData, Position, SlotDisplay, Tag,
    VarInt, UUID,
};

pub trait Packet: MinecraftData {
    const ID: VarInt;

    /// wrapper around Self::decode so that the interface is more symmetric
    fn decode_packet<R: Read>(reader: &mut R) -> Result<Self, Error> {
        Self::decode(reader)
    }

    fn encode_packet<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        let len = Self::ID.num_bytes() + self.num_bytes();
        VarInt(len as i32).encode(writer)?;
        Self::ID.encode(writer)?;
        self.encode(writer)?;
        Ok(())
    }
}

pub fn decode_packet_header<R: Read>(reader: &mut R) -> Result<PacketHeader, Error> {
    let len = VarInt::decode(reader)?;
    let id = VarInt::decode(reader)?;
    Ok(PacketHeader { len, id })
}

#[derive(Debug, Clone, Copy)]
pub struct PacketHeader {
    pub len: VarInt,
    pub id: VarInt,
}

#[derive(Debug, Clone, Copy, MinecraftData)]
pub enum HandshakeIntent {
    #[mc_repr(VarInt(1))]
    Status,
    #[mc_repr(VarInt(2))]
    Login,
    #[mc_repr(VarInt(3))]
    Transfer,
}

#[derive(Debug, Clone, MinecraftData)]
pub struct HandshakePacket {
    pub protocol_version: VarInt,
    pub server_address: MString<255>,
    pub server_port: u16,
    pub intent: HandshakeIntent,
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

#[derive(Debug, Clone, MinecraftData)]
pub struct LoginStartPacket {
    pub name: MString<16>,
    pub uuid: UUID,
}

impl Packet for LoginStartPacket {
    const ID: VarInt = VarInt(0x00);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct EncryptionRequestPacket {
    pub server_id: MString<20>,
    pub public_key: Vec<u8>,
    pub verify_token: Vec<u8>,
    pub should_authenticate: bool,
}

impl Packet for EncryptionRequestPacket {
    const ID: VarInt = VarInt(0x01);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct LoginSuccessPacket {
    pub client: GameProfile,
}

impl Packet for LoginSuccessPacket {
    const ID: VarInt = VarInt(0x02);
}

#[derive(Debug, Clone, Copy, MinecraftData)]
pub struct LoginAcknowledgedPacket;

impl Packet for LoginAcknowledgedPacket {
    const ID: VarInt = VarInt(0x03);
}

#[derive(Debug, Clone)]
pub enum PluginChannelData {
    MinecraftBrand(MString<32767>),
    Unknown(Identifier),
}

impl PluginChannelData {
    fn identifier(&self) -> Identifier {
        match self {
            Self::MinecraftBrand(_) => Identifier::try_from("minecraft:brand".to_owned()).unwrap(),
            Self::Unknown(id) => id.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClientboundConfigurationPluginMessagePacket {
    pub data: PluginChannelData,
}

impl MinecraftData for ClientboundConfigurationPluginMessagePacket {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let channel = Identifier::decode(reader)?;
        let data = match channel.to_string().as_str() {
            "minecraft:brand" => PluginChannelData::MinecraftBrand(MString::decode(reader)?),
            _ => PluginChannelData::Unknown(channel),
        };
        Ok(Self { data })
    }

    fn encode<W: Write>(self, writer: &mut W) -> Result<(), Error> {
        self.data.identifier().encode(writer)?;
        match self.data {
            PluginChannelData::MinecraftBrand(brand) => {
                brand.encode(writer)?;
            }
            PluginChannelData::Unknown(_) => {}
        }
        Ok(())
    }

    fn num_bytes(&self) -> usize {
        self.data.identifier().num_bytes()
            + match &self.data {
                PluginChannelData::MinecraftBrand(brand) => brand.num_bytes(),
                PluginChannelData::Unknown(_) => 0,
            }
    }
}

impl Packet for ClientboundConfigurationPluginMessagePacket {
    const ID: VarInt = VarInt(0x01);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct FeatureFlagsPacket {
    feature_flags: Vec<Identifier>,
}

impl Packet for FeatureFlagsPacket {
    const ID: VarInt = VarInt(0x0C);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct DataPack {
    pub namespace: MString<32767>,
    pub id: MString<32767>,
    pub version: MString<32767>,
}

#[derive(Debug, Clone, MinecraftData)]
pub struct ClientboundKnownPacksPacket {
    pub known_packs: Vec<DataPack>,
}

impl Packet for ClientboundKnownPacksPacket {
    const ID: VarInt = VarInt(0x0E);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct ServerboundKnownPacksPacket {
    pub known_packs: Vec<DataPack>,
}

impl Packet for ServerboundKnownPacksPacket {
    const ID: VarInt = VarInt(0x07);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct ConfigurationKeepAlivePacket {
    pub keep_alive_id: i64,
}

impl Packet for ConfigurationKeepAlivePacket {
    const ID: VarInt = VarInt(0x04);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct RegistryEntry {
    pub id: Identifier,
    pub data: Option<u8>, // TODO: actually NBT
}

#[derive(Debug, Clone, MinecraftData)]
pub struct RegistryDataPacket {
    pub registry_id: Identifier,
    pub entries: Vec<RegistryEntry>,
}

impl Packet for RegistryDataPacket {
    const ID: VarInt = VarInt(0x07);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct TaggedRegistry {
    pub registry: Identifier,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, MinecraftData)]
pub struct ConfigurationUpdateTagsPacket {
    tagged_registries: Vec<TaggedRegistry>,
}

impl Packet for ConfigurationUpdateTagsPacket {
    const ID: VarInt = VarInt(0x0D);
}

#[derive(Debug, Clone, Copy, MinecraftData)]
pub struct FinishConfigurationPacket;

impl Packet for FinishConfigurationPacket {
    const ID: VarInt = VarInt(0x03);
}

#[derive(Debug, Clone, Copy, MinecraftData)]
pub struct AcknowledgeFinishConfigurationPacket;

impl Packet for AcknowledgeFinishConfigurationPacket {
    const ID: VarInt = VarInt(0x03);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct ClientboundPlayKeepAlivePacket {
    pub keep_alive_id: i64,
}

impl Packet for ClientboundPlayKeepAlivePacket {
    const ID: VarInt = VarInt(0x2B);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct ServerboundPlayKeepAlivePacket {
    pub keep_alive_id: i64,
}

impl Packet for ServerboundPlayKeepAlivePacket {
    const ID: VarInt = VarInt(0x1B);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct PlayLoginPacket {
    entity_id: i32,
    is_hardcore: bool,
    dimension_names: Vec<Identifier>,
    max_players: VarInt,
    view_distance: VarInt,
    simulation_distance: VarInt,
    reduced_debug_info: bool,
    enable_respawn_screen: bool,
    do_limited_crafting: bool,
    dimension_type: VarInt,
    dimension_name: Identifier,
    hashed_seed: i64,
    game_mode: u8,
    previous_game_mode: i8,
    is_debug: bool,
    is_flat: bool,
    has_death_location: bool,
    #[present_if(has_death_location)]
    death_dimention_name: Option<Identifier>,
    #[present_if(has_death_location)]
    death_location: Option<Position>,
    portal_cooldown: VarInt,
    sea_level: VarInt,
    enforced_secure_chat: bool,
}

impl Packet for PlayLoginPacket {
    const ID: VarInt = VarInt(0x30);
}

#[derive(Debug, Clone, Copy, MinecraftData)]
pub enum Difficulty {
    #[mc_repr(VarInt(0))]
    Peaceful,
    #[mc_repr(VarInt(1))]
    Easy,
    #[mc_repr(VarInt(2))]
    Normal,
    #[mc_repr(VarInt(3))]
    Hard,
}

#[derive(Debug, Clone, MinecraftData)]
pub struct ChangeDifficultyPacket {
    pub difficulty: Difficulty,
    pub difficulty_locked: bool,
}

impl Packet for ChangeDifficultyPacket {
    const ID: VarInt = VarInt(0x0A);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct ClientboundPlayerAbilitiesPacket {
    pub flags: u8,
    pub flying_speed: f32,
    pub fov_modifier: f32,
}

impl Packet for ClientboundPlayerAbilitiesPacket {
    const ID: VarInt = VarInt(0x3E);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct SetHealthPacket {
    pub health: f32,
    pub food: VarInt,
    pub food_saturation: f32,
}

impl Packet for SetHealthPacket {
    const ID: VarInt = VarInt(0x66);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct ClientboundSetHeldItemPacket {
    pub slot: VarInt,
}

impl Packet for ClientboundSetHeldItemPacket {
    const ID: VarInt = VarInt(0x67);
}

#[derive(Debug, Clone, MinecraftData)]
pub struct PropertySet {
    pub id: Identifier,
    pub items: Vec<VarInt>,
}

#[derive(Debug, Clone, MinecraftData)]
pub struct StonecutterRecipe {
    pub ingredients: IDSet,
    pub slot_display: SlotDisplay,
}

#[derive(Debug, Clone, MinecraftData)]
pub struct UpdateRecipesPacket {
    pub property_sets: Vec<PropertySet>,
    pub stonecutter_recipes: Vec<StonecutterRecipe>,
}

impl Packet for UpdateRecipesPacket {
    const ID: VarInt = VarInt(0x83);
}
