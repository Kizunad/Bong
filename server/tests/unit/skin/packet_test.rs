use bong_server::skin::packet::{NpcPlayerInfoRemoveS2c, NpcPlayerInfoUpdateS2c};
use bong_server::skin::{SignedSkin, SkinSource};
use valence::prelude::Uuid;
use valence::protocol::packets::play::{PlayerListS2c, PlayerRemoveS2c};
use valence::protocol::{Encode, Packet};

fn test_skin() -> SignedSkin {
    SignedSkin {
        value: "skin-value".to_string(),
        signature: "skin-signature".to_string(),
        source: SkinSource::MineSkinRandom {
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
    };

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
