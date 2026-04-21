package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InspectScreenMoveIntentTest {

    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
    }

    private void install() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void dispatchMoveIntentSendsForInventoryBackedLocations() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1001L,
            "starter_talisman",
            "启程护符",
            1,
            1,
            0.2,
            "uncommon",
            "初入修途者配发的护身符。",
            1,
            0.76,
            0.93
        );

        screen.dispatchMoveIntent(
            item,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0),
            new ClientRequestProtocol.HotbarLoc(3)
        );

        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":1001,\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":0,\"col\":0},\"to\":{\"kind\":\"hotbar\",\"index\":3}}",
            sent.get(0).body()
        );
    }

    @Test
    void dispatchMoveIntentSkipsWhenSourceLocationIsUnsupported() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1001L,
            "starter_talisman",
            "启程护符",
            1,
            1,
            0.2,
            "uncommon",
            "初入修途者配发的护身符。",
            1,
            0.76,
            0.93
        );

        screen.dispatchMoveIntent(item, null, new ClientRequestProtocol.HotbarLoc(3));

        assertTrue(sent.isEmpty());
    }

    @Test
    void dispatchMoveIntentSkipsMockItemsWithoutAuthoritativeInstanceId() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.create(
            "spirit_grass",
            "灵草",
            1,
            1,
            0.2,
            "common",
            "用于测试的 mock 物品。"
        );

        screen.dispatchMoveIntent(
            item,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0),
            new ClientRequestProtocol.HotbarLoc(3)
        );

        assertTrue(sent.isEmpty());
    }

    @Test
    void dispatchDiscardIntentSendsForInventoryBackedLocations() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1001L,
            "starter_talisman",
            "启程护符",
            1,
            1,
            0.2,
            "uncommon",
            "初入修途者配发的护身符。",
            1,
            0.76,
            0.93
        );

        assertTrue(screen.dispatchDiscardIntent(
            item,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0)
        ));

        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"inventory_discard_item\",\"v\":1,\"instance_id\":1001,\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":0,\"col\":0}}",
            sent.get(0).body()
        );
    }

    @Test
    void dispatchDiscardIntentSkipsWhenSourceLocationIsUnsupported() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1001L,
            "starter_talisman",
            "启程护符",
            1,
            1,
            0.2,
            "uncommon",
            "初入修途者配发的护身符。",
            1,
            0.76,
            0.93
        );

        assertTrue(!screen.dispatchDiscardIntent(item, null));

        assertTrue(sent.isEmpty());
    }

    @Test
    void dispatchDiscardIntentSkipsMockItemsWithoutAuthoritativeInstanceId() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.create(
            "spirit_grass",
            "灵草",
            1,
            1,
            0.2,
            "common",
            "用于测试的 mock 物品。"
        );

        assertTrue(!screen.dispatchDiscardIntent(
            item,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0)
        ));

        assertTrue(sent.isEmpty());
    }
}
