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
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-tarkov-backpack-v1 P3 —— InspectScreen 穿戴背包件视图相关行为单测。
 *
 * <p>覆盖：拖入穿戴背包件视图走
 * {@code sendInventoryMove}（断言 type=inventory_move_intent，<b>非</b> external_container_move，
 * 且 from/to payload 结构正确）。</p>
 */
public class InspectScreenWornContainerTest {

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


    // ── 拖入穿戴背包件视图发包路线（交付物 #1/#4：sendInventoryMove，非 sendExternalContainerMove） ──

    @Test
    void wornContainerDropDispatchesInventoryMoveNotExternalContainerMove() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem dragged = InventoryItem.createFull(
            1008L, "spirit_herb", "灵草", 1, 1, 0.1, "common", "", 1, 1.0, 1.0);

        // 拖入穿戴背包件视图 = move 到 ContainerLoc(pack_<id>, row, col)，from 为来源格子。
        // 这正是 attemptDrop 的 wornContainerPanel 分支所走的派发；硬约束此路必须走 sendInventoryMove。
        screen.dispatchMoveIntent(
            dragged,
            new ClientRequestProtocol.ContainerLoc("main_pack", 2, 3),
            new ClientRequestProtocol.ContainerLoc("pack_1007", 0, 1),
            false
        );

        assertEquals(1, sent.size(), "应发出且仅发出一条 move intent");
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());

        String body = sent.get(0).body();
        // 关键断言：type 是 inventory_move_intent，绝不是 external_container_move（loot 专用，带 session_id）。
        assertTrue(body.contains("\"type\":\"inventory_move_intent\""),
            "穿戴背包件拖入必须走 inventory_move_intent，实际 payload = " + body);
        assertFalse(body.contains("external_container_move"),
            "严禁走 external_container_move（loot 专用），实际 payload = " + body);
        assertFalse(body.contains("session_id"),
            "inventory_move_intent 不应携带 session_id，实际 payload = " + body);
        // from/to payload 结构：to 指向 pack_<id> 容器。
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":1008,"
                + "\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":2,\"col\":3},"
                + "\"to\":{\"kind\":\"container\",\"container_id\":\"pack_1007\",\"row\":0,\"col\":1}}",
            body
        );
    }

    @Test
    void wornContainerDragOutDispatchesInventoryMoveFromPackContainer() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem dragged = InventoryItem.createFull(
            1008L, "spirit_herb", "灵草", 1, 1, 0.1, "common", "", 1, 1.0, 1.0);

        // 从穿戴背包件视图拖出到主背包 = from 为 pack_<id> 容器，to 为 main_pack。
        screen.dispatchMoveIntent(
            dragged,
            new ClientRequestProtocol.ContainerLoc("pack_1007", 0, 0),
            new ClientRequestProtocol.ContainerLoc("main_pack", 1, 1),
            false
        );

        assertEquals(1, sent.size());
        String body = sent.get(0).body();
        assertTrue(body.contains("\"type\":\"inventory_move_intent\""));
        assertFalse(body.contains("external_container_move"));
        assertTrue(body.contains("\"from\":{\"kind\":\"container\",\"container_id\":\"pack_1007\""),
            "拖出来源应是 pack_1007 容器，实际 payload = " + body);
    }

    @Test
    void unequipPouchToBodyPocketDispatchesEquipToContainerMove() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem pouch = InventoryItem.createFull(
            7001L, "grass_pouch", "小草包", 2, 2, 0.3, "common", "", 1, 0.0, 1.0);

        // 卸下胸部 worn 层的小草包到贴身口袋，必须保持 inventory_move_intent 路由。
        screen.dispatchMoveIntent(
            pouch,
            new ClientRequestProtocol.EquipLoc("chest", "worn"),
            new ClientRequestProtocol.ContainerLoc("body_pocket", 0, 0),
            false
        );

        assertEquals(1, sent.size(), "卸包应发出且仅发出一条 move intent");
        String body = sent.get(0).body();
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":7001,"
                + "\"from\":{\"kind\":\"equip\",\"slot\":\"chest\",\"state\":\"worn\"},"
                + "\"to\":{\"kind\":\"container\",\"container_id\":\"body_pocket\",\"row\":0,\"col\":0}}",
            body,
            "卸包必须发送 EquipLoc(chest,worn) → ContainerLoc(body_pocket,0,0)"
        );
    }
}
