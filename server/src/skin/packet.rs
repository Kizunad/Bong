use std::borrow::Cow;
use std::io::Write;
use std::ops::DerefMut;

use valence::prelude::{Client, GameMode, Uuid};
use valence::protocol::encode::WritePacket;
use valence::protocol::packets::play::{player_list_s2c as packet, PlayerListS2c, PlayerRemoveS2c};
use valence::protocol::profile::Property;
use valence::protocol::{anyhow, Encode, Packet, PacketSide, PacketState};

use super::SignedSkin;

#[derive(Clone, Debug)]
pub struct NpcPlayerInfoUpdateS2c<'a> {
    pub uuid: Uuid,
    pub name: &'a str,
    pub skin: &'a SignedSkin,
}

impl<'a> NpcPlayerInfoUpdateS2c<'a> {
    pub fn write_to(&self, client: &mut Client) {
        client.write_packet(self);
    }

    fn to_valence_packet(&self) -> PlayerListS2c<'a> {
        let property = Property {
            name: "textures".to_string(),
            value: self.skin.value.clone(),
            signature: (!self.skin.signature.is_empty()).then(|| self.skin.signature.clone()),
        };
        let entry = packet::PlayerListEntry {
            player_uuid: self.uuid,
            username: self.name,
            properties: Cow::Owned(vec![property]),
            chat_data: None,
            listed: false,
            ping: 0,
            game_mode: GameMode::Survival,
            display_name: None,
        };

        PlayerListS2c {
            actions: npc_player_list_actions(),
            entries: Cow::Owned(vec![entry]),
        }
    }
}

impl Encode for NpcPlayerInfoUpdateS2c<'_> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.to_valence_packet().encode(w)
    }
}

impl Packet for NpcPlayerInfoUpdateS2c<'_> {
    const ID: i32 = PlayerListS2c::ID;
    const NAME: &'static str = "bong:npc_player_info_update_s2c";
    const SIDE: PacketSide = PacketSide::Clientbound;
    const STATE: PacketState = PacketState::Play;
}

#[derive(Clone, Copy, Debug)]
pub struct NpcPlayerInfoRemoveS2c {
    pub uuid: Uuid,
}

impl NpcPlayerInfoRemoveS2c {
    pub fn write_to(&self, client: &mut Client) {
        client.write_packet(self);
    }
}

impl Encode for NpcPlayerInfoRemoveS2c {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        PlayerRemoveS2c {
            uuids: Cow::Owned(vec![self.uuid]),
        }
        .encode(w)
    }
}

impl Packet for NpcPlayerInfoRemoveS2c {
    const ID: i32 = PlayerRemoveS2c::ID;
    const NAME: &'static str = "bong:npc_player_info_remove_s2c";
    const SIDE: PacketSide = PacketSide::Clientbound;
    const STATE: PacketState = PacketState::Play;
}

pub fn npc_player_list_actions() -> packet::PlayerListActions {
    packet::PlayerListActions::new()
        .with_add_player(true)
        .with_update_game_mode(true)
        .with_update_listed(true)
        .with_update_latency(true)
        .with_update_display_name(true)
}

pub fn broadcast_add_player<C>(
    clients: impl IntoIterator<Item = C>,
    npc_uuid: Uuid,
    name: &str,
    skin: &SignedSkin,
) where
    C: DerefMut<Target = Client>,
{
    let packet = NpcPlayerInfoUpdateS2c {
        uuid: npc_uuid,
        name,
        skin,
    };
    for mut client in clients {
        packet.write_to(&mut client);
    }
}

pub fn send_add_player(client: &mut Client, npc_uuid: Uuid, name: &str, skin: &SignedSkin) {
    NpcPlayerInfoUpdateS2c {
        uuid: npc_uuid,
        name,
        skin,
    }
    .write_to(client);
}

pub fn broadcast_remove_player<C>(clients: impl IntoIterator<Item = C>, npc_uuid: Uuid)
where
    C: DerefMut<Target = Client>,
{
    for mut client in clients {
        NpcPlayerInfoRemoveS2c { uuid: npc_uuid }.write_to(&mut client);
    }
}

#[allow(dead_code)]
pub fn send_remove_player(client: &mut Client, npc_uuid: Uuid) {
    NpcPlayerInfoRemoveS2c { uuid: npc_uuid }.write_to(client);
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::protocol::{Encode, Packet};

    fn test_skin() -> SignedSkin {
        SignedSkin {
            value: "skin-value".to_string(),
            signature: "skin-signature".to_string(),
            source: crate::skin::SkinSource::MineSkinRandom {
                hash: "hash".into(),
            },
        }
    }

    #[test]
    fn player_info_packet_matches_protocol_field_order() {
        let uuid = Uuid::from_u128(0x0102030405060708090a0b0c0d0e0f10);
        let skin = test_skin();
        let packet = NpcPlayerInfoUpdateS2c {
            uuid,
            name: "npc_0001",
            skin: &skin,
        }
        .to_valence_packet();

        let mut body = Vec::new();
        packet.encode(&mut body).unwrap();

        let mut expected = Vec::new();
        expected.push(0x3d); // AddPlayer + GameMode + Listed + Latency + DisplayName.
        expected.push(0x01); // one entry
        expected.extend_from_slice(&uuid.as_u128().to_be_bytes());
        expected.push(0x08);
        expected.extend_from_slice(b"npc_0001");
        expected.push(0x01);
        expected.push(0x08);
        expected.extend_from_slice(b"textures");
        expected.push(0x0a);
        expected.extend_from_slice(b"skin-value");
        expected.push(0x01);
        expected.push(0x0e);
        expected.extend_from_slice(b"skin-signature");
        expected.push(0x00); // survival
        expected.push(0x00); // listed=false
        expected.push(0x00); // latency
        expected.push(0x00); // no display name

        assert_eq!(body, expected);
    }

    #[test]
    fn player_info_packet_id_is_mc_1_20_1_player_list_update() {
        assert_eq!(NpcPlayerInfoUpdateS2c::ID, PlayerListS2c::ID);
        assert_eq!(NpcPlayerInfoRemoveS2c::ID, PlayerRemoveS2c::ID);
        assert_eq!(PlayerListS2c::ID, 0x3a);
        assert_eq!(PlayerRemoveS2c::ID, 0x39);
    }
}
