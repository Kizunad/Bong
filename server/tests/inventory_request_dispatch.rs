use bong_server::network::client_request::inventory_contract::{
    try_into_inventory_request, InventoryRequest,
};
use bong_server::schema::client_request::ClientRequestV1;
use bong_server::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};

fn source() -> InventoryLocationV1 {
    InventoryLocationV1::Container {
        container_id: "body_pocket".to_owned(),
        row: u64::MAX,
        col: 17,
    }
}

fn destination() -> InventoryLocationV1 {
    InventoryLocationV1::Equip {
        slot: EquipSlotV1::ExtraHand1,
        state: EquipStateV1::Held,
    }
}

#[test]
fn inventory_extractor_accepts_all_six_variants_and_preserves_fields() {
    let cases = [
        (
            ClientRequestV1::InventoryMoveIntent {
                v: 1,
                instance_id: u64::MAX,
                from: source(),
                to: destination(),
                rotated: true,
            },
            InventoryRequest::InventoryMoveIntent {
                instance_id: u64::MAX,
                from: source(),
                to: destination(),
                rotated: true,
            },
        ),
        (
            ClientRequestV1::InventoryDiscardItem {
                v: 1,
                instance_id: 42,
                from: source(),
            },
            InventoryRequest::InventoryDiscardItem {
                instance_id: 42,
                from: source(),
            },
        ),
        (
            ClientRequestV1::PickupDroppedItem {
                v: 1,
                instance_id: 43,
            },
            InventoryRequest::PickupDroppedItem { instance_id: 43 },
        ),
        (
            ClientRequestV1::ContainerOpen {
                v: 1,
                entity_id: i32::MIN,
            },
            InventoryRequest::ContainerOpen {
                entity_id: i32::MIN,
            },
        ),
        (
            ClientRequestV1::ExternalContainerMove {
                v: 1,
                session_id: u64::MAX,
                instance_id: 44,
                from: source(),
                to: destination(),
            },
            InventoryRequest::ExternalContainerMove {
                session_id: u64::MAX,
                instance_id: 44,
                from: source(),
                to: destination(),
            },
        ),
        (
            ClientRequestV1::ExternalContainerClose {
                v: 1,
                session_id: 45,
            },
            InventoryRequest::ExternalContainerClose { session_id: 45 },
        ),
    ];

    for (wire, expected) in cases {
        assert_eq!(
            try_into_inventory_request(wire).ok(),
            Some(expected),
            "inventory extractor must preserve every field for each of the six variants"
        );
    }
}

#[test]
fn non_inventory_request_is_returned_unchanged() {
    let request = ClientRequestV1::ScrollReadRequest {
        v: 1,
        instance_id: u64::MAX,
    };
    assert!(matches!(
        try_into_inventory_request(request),
        Err(ClientRequestV1::ScrollReadRequest {
            v: 1,
            instance_id: u64::MAX
        })
    ));
}
