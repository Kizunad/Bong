package com.bong.client.network;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class ClientRequestProtocolTest {

    @Test
    void encodesSetMeridianTarget() {
        String json = ClientRequestProtocol.encodeSetMeridianTarget(
            ClientRequestProtocol.MeridianId.Lung
        );
        assertEquals(
            "{\"type\":\"set_meridian_target\",\"v\":1,\"meridian\":\"Lung\"}",
            json
        );
    }

    @Test
    void encodesBreakthroughRequest() {
        String json = ClientRequestProtocol.encodeBreakthroughRequest();
        assertEquals("{\"type\":\"breakthrough_request\",\"v\":1}", json);
    }

    @Test
    void encodesForgeRequestWithRateAxis() {
        String json = ClientRequestProtocol.encodeForgeRequest(
            ClientRequestProtocol.MeridianId.Ren,
            ClientRequestProtocol.ForgeAxis.Rate
        );
        assertEquals(
            "{\"type\":\"forge_request\",\"v\":1,\"meridian\":\"Ren\",\"axis\":\"Rate\"}",
            json
        );
    }

    @Test
    void encodesForgeRequestWithCapacityAxis() {
        String json = ClientRequestProtocol.encodeForgeRequest(
            ClientRequestProtocol.MeridianId.Du,
            ClientRequestProtocol.ForgeAxis.Capacity
        );
        assertEquals(
            "{\"type\":\"forge_request\",\"v\":1,\"meridian\":\"Du\",\"axis\":\"Capacity\"}",
            json
        );
    }

    @Test
    void encodesApplyPillSelf() {
        String json = ClientRequestProtocol.encodeApplyPillSelf(1001L);
        assertEquals(
            "{\"type\":\"apply_pill\",\"v\":1,\"instance_id\":1001,\"target\":{\"kind\":\"self\"}}",
            json
        );
    }

    @Test
    void encodesApplyPillMeridianTarget() {
        String json = ClientRequestProtocol.encodeApplyPill(
            2002L,
            new ClientRequestProtocol.MeridianTarget(ClientRequestProtocol.MeridianId.Ren)
        );
        assertEquals(
            "{\"type\":\"apply_pill\",\"v\":1,\"instance_id\":2002,\"target\":{\"kind\":\"meridian\",\"meridian_id\":\"Ren\"}}",
            json
        );
    }

    @Test
    void encodesInventoryMoveFromContainerToHotbar() {
        String json = ClientRequestProtocol.encodeInventoryMove(
            1001L,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0),
            new ClientRequestProtocol.HotbarLoc(3)
        );
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":1001,\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":0,\"col\":0},\"to\":{\"kind\":\"hotbar\",\"index\":3}}",
            json
        );
    }

    @Test
    void encodesInventoryMoveFromEquipToContainer() {
        String json = ClientRequestProtocol.encodeInventoryMove(
            2002L,
            new ClientRequestProtocol.EquipLoc("main_hand"),
            new ClientRequestProtocol.ContainerLoc("small_pouch", 1, 2)
        );
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":2002,\"from\":{\"kind\":\"equip\",\"slot\":\"main_hand\"},\"to\":{\"kind\":\"container\",\"container_id\":\"small_pouch\",\"row\":1,\"col\":2}}",
            json
        );
    }

    @Test
    void encodesPickupDroppedItem() {
        String json = ClientRequestProtocol.encodePickupDroppedItem(3003L);
        assertEquals(
            "{\"type\":\"pickup_dropped_item\",\"v\":1,\"instance_id\":3003}",
            json
        );
    }

    @Test
    void encodesInventoryDiscardItem() {
        String json = ClientRequestProtocol.encodeInventoryDiscardItem(
            1001L,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0)
        );
        assertEquals(
            "{\"type\":\"inventory_discard_item\",\"v\":1,\"instance_id\":1001,\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":0,\"col\":0}}",
            json
        );
    }

    @Test
    void meridianIdEnumCoversAll20Channels() {
        // 12 正经 + 8 奇经
        assertEquals(20, ClientRequestProtocol.MeridianId.values().length);
    }

    @Test
    void toMeridianIdMapsAllChannelsExhaustively() {
        // 所有 20 条 UI 通道均能映射为服务端 id，不抛 MatchException
        for (com.bong.client.inventory.model.MeridianChannel ch :
                com.bong.client.inventory.model.MeridianChannel.values()) {
            ClientRequestProtocol.MeridianId id = ClientRequestProtocol.toMeridianId(ch);
            assertEquals(true, id != null, "missing mapping for " + ch);
        }
    }

    @Test
    void encodesInsightDecisionChosen() {
        String json = ClientRequestProtocol.encodeInsightDecision("awaken_first", 2);
        assertEquals(
            "{\"type\":\"insight_decision\",\"v\":1,\"trigger_id\":\"awaken_first\",\"choice_idx\":2}",
            json
        );
    }

    @Test
    void encodesInsightDecisionDeclinedAsNull() {
        String json = ClientRequestProtocol.encodeInsightDecision("awaken_first", null);
        assertEquals(
            "{\"type\":\"insight_decision\",\"v\":1,\"trigger_id\":\"awaken_first\",\"choice_idx\":null}",
            json
        );
    }

    @Test
    void toMeridianIdMapsSampleChannels() {
        assertEquals(ClientRequestProtocol.MeridianId.Heart,
            ClientRequestProtocol.toMeridianId(com.bong.client.inventory.model.MeridianChannel.HT));
        assertEquals(ClientRequestProtocol.MeridianId.Ren,
            ClientRequestProtocol.toMeridianId(com.bong.client.inventory.model.MeridianChannel.REN));
        assertEquals(ClientRequestProtocol.MeridianId.YinWei,
            ClientRequestProtocol.toMeridianId(com.bong.client.inventory.model.MeridianChannel.YIN_WEI));
        assertEquals(ClientRequestProtocol.MeridianId.TripleEnergizer,
            ClientRequestProtocol.toMeridianId(com.bong.client.inventory.model.MeridianChannel.TE));
    }
}
