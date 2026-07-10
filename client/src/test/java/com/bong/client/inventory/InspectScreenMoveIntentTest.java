package com.bong.client.inventory;

import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.component.EquipmentPanel;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.SlotContents;
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

public class InspectScreenMoveIntentTest {

    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        QuickUseSlotStore.resetForTests();
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
            new ClientRequestProtocol.HotbarLoc(3),
            false
        );

        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":1001,\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":0,\"col\":0},\"to\":{\"kind\":\"hotbar\",\"index\":3}}",
            sent.get(0).body()
        );
    }

    // plan-rotate-v1 — dispatchMoveIntent 透传 rotated=true 时 payload 携带 rotated 字段。
    @Test
    void dispatchMoveIntentPassesRotatedFlagThrough() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1002L, "long_rod", "长杆", 2, 1, 0.5, "common", "旋转测试物", 1, 1.0, 1.0);

        screen.dispatchMoveIntent(
            item,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0),
            new ClientRequestProtocol.ContainerLoc("main_pack", 2, 3),
            true
        );

        assertEquals(1, sent.size());
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":1002,"
                + "\"from\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":0,\"col\":0},"
                + "\"to\":{\"kind\":\"container\",\"container_id\":\"main_pack\",\"row\":2,\"col\":3},"
                + "\"rotated\":true}",
            sent.get(0).body(),
            "旋转落位必须把 rotated:true 发给 server（否则 server 不会互换 grid_w/grid_h）"
        );
    }

    @Test
    void packWindowDropDispatchesContainerLocViaInventoryMove() {
        // plan-tarkov-floating-windows 交付物 #4/#5：套包悬浮窗（含跨窗）落位走 sendInventoryMove，
        // to=ContainerLoc(pack_<id>,row,col)；硬约束 —— 绝不走 sendExternalContainerMove（loot 专用）。
        // attemptDrop 的 owo hit-test 需真布局，此处直接锁 dispatchMoveIntent 的发包契约（loop 内同一行）。
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1007L, "spirit_herb", "灵草", 1, 1, 0.2, "common", "测试货物", 1, 1.0, 1.0);

        // 模拟从 pack_A grid 拖到 pack_B grid（跨窗）：from=pack_A, to=pack_B。
        screen.dispatchMoveIntent(
            item,
            new ClientRequestProtocol.ContainerLoc("pack_500", 0, 0),
            new ClientRequestProtocol.ContainerLoc("pack_900", 1, 2),
            false
        );

        assertEquals(1, sent.size(), "应发出一条 move intent");
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"inventory_move_intent\",\"v\":1,\"instance_id\":1007,"
                + "\"from\":{\"kind\":\"container\",\"container_id\":\"pack_500\",\"row\":0,\"col\":0},"
                + "\"to\":{\"kind\":\"container\",\"container_id\":\"pack_900\",\"row\":1,\"col\":2}}",
            sent.get(0).body(),
            "跨窗落位应 = inventory_move_intent，to 用目标窗自己的 pack_<id>；payload 非 external move");
        assertTrue(sent.get(0).body().contains("inventory_move_intent"),
            "必须是 inventory_move_intent（走 sendInventoryMove），绝非 external_container_move");
        assertFalse(sent.get(0).body().contains("external"),
            "硬约束：套包窗落位绝不走 sendExternalContainerMove（loot 专用，需 session_id）");
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

        screen.dispatchMoveIntent(item, null, new ClientRequestProtocol.HotbarLoc(3), false);

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
            new ClientRequestProtocol.HotbarLoc(3),
            false
        );

        assertTrue(sent.isEmpty());
    }

    @Test
    void shiftQuickEquipRoutesOccupiedMainHandSwordToExtraHandAndDispatchesIntent() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        BackpackGridPanel grid = new BackpackGridPanel("main_pack", 3, 3);
        EquipmentPanel panel = new EquipmentPanel();
        InventoryItem equippedMain = item(2001L, "bone_dagger");
        InventoryItem sword = item(2002L, "iron_sword");
        panel.slotFor(EquipSlotType.MAIN_HAND).setContents(SlotContents.ofHeld(equippedMain));
        grid.place(sword, 1, 1);
        screen.configureEquipInteractionForTests(grid, panel);

        screen.quickEquipFromGridForTests(sword);

        assertEquals(
            null,
            grid.itemAt(1, 1),
            "expected shift quick-equip to remove sword from source grid because dispatch succeeded"
        );
        assertEquals(
            sword,
            panel.slotFor(EquipSlotType.EXTRA_HAND_0).contents().held(),
            "expected occupied main hand and ineligible off hand to route sword into EXTRA_HAND_0"
        );
        assertEquals(
            1,
            sent.size(),
            "expected shift quick-equip to dispatch one InventoryMoveIntent, actual " + sent.size()
        );
        assertTrue(
            sent.get(0).body().contains(
                "\"to\":{\"kind\":\"equip\",\"slot\":\"extra_hand_0\",\"state\":\"held\"}"
            ),
            "expected quick-equip payload to target EXTRA_HAND_0 held, actual " + sent.get(0).body()
        );
    }

    @Test
    void shiftQuickEquipWithNoLegalHandLeavesGridAndSendsNothing() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        BackpackGridPanel grid = new BackpackGridPanel("main_pack", 3, 3);
        EquipmentPanel panel = new EquipmentPanel();
        InventoryItem sword = item(2100L, "iron_sword");
        panel.slotFor(EquipSlotType.MAIN_HAND)
            .setContents(SlotContents.ofHeld(item(2101L, "bone_dagger")));
        panel.slotFor(EquipSlotType.EXTRA_HAND_0)
            .setContents(SlotContents.ofHeld(item(2102L, "bone_dagger")));
        panel.slotFor(EquipSlotType.EXTRA_HAND_1)
            .setContents(SlotContents.ofHeld(item(2103L, "bone_dagger")));
        grid.place(sword, 0, 0);
        screen.configureEquipInteractionForTests(grid, panel);

        screen.quickEquipFromGridForTests(sword);

        assertEquals(
            sword,
            grid.itemAt(0, 0),
            "expected source item to remain because no legal held slot exists"
        );
        assertTrue(
            sent.isEmpty(),
            "expected no InventoryMoveIntent when all legal hand slots are occupied, actual " + sent.size()
        );
    }

    @Test
    void dragEquipCommitTargetsExtraHandOneAndDispatchesIntent() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        EquipmentPanel panel = new EquipmentPanel();
        screen.configureEquipInteractionForTests(null, panel);
        InventoryItem tool = item(2200L, "stone_pickaxe");

        boolean committed = screen.commitEquipDrop(
            tool,
            new ClientRequestProtocol.ContainerLoc("main_pack", 2, 0),
            EquipSlotType.EXTRA_HAND_1
        );

        assertTrue(committed, "expected valid drag drop into EXTRA_HAND_1 to commit, actual false");
        assertEquals(
            tool,
            panel.slotFor(EquipSlotType.EXTRA_HAND_1).contents().held(),
            "expected drag drop to update EXTRA_HAND_1 held contents before authoritative refresh"
        );
        assertEquals(
            1,
            sent.size(),
            "expected valid extra-hand drag to dispatch one InventoryMoveIntent, actual " + sent.size()
        );
        assertTrue(
            sent.get(0).body().contains(
                "\"to\":{\"kind\":\"equip\",\"slot\":\"extra_hand_1\",\"state\":\"held\"}"
            ),
            "expected drag payload to target EXTRA_HAND_1 held, actual " + sent.get(0).body()
        );
    }

    @Test
    void dragEquipCommitAppendsBodyWornStackAndDispatchesWornIntent() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        EquipmentPanel panel = new EquipmentPanel();
        InventoryItem existing = item(2250L, "armor_bone_chestplate");
        InventoryItem dragged = item(2251L, "armor_iron_chestplate");
        panel.slotFor(EquipSlotType.CHEST).setContents(SlotContents.ofWorn(existing));
        screen.configureEquipInteractionForTests(null, panel);

        boolean committed = screen.commitEquipDrop(
            dragged,
            new ClientRequestProtocol.ContainerLoc("main_pack", 1, 2),
            EquipSlotType.CHEST
        );

        assertTrue(committed, "expected valid body-slot drag to commit, actual false");
        assertEquals(
            List.of(existing, dragged),
            panel.slotFor(EquipSlotType.CHEST).contents().worn(),
            "expected body-slot commit to append the dragged armor as worn stack top"
        );
        assertEquals(
            null,
            panel.slotFor(EquipSlotType.CHEST).contents().held(),
            "expected body-slot commit to preserve held as null"
        );
        assertEquals(
            1,
            sent.size(),
            "expected valid body-slot drag to dispatch one InventoryMoveIntent, actual " + sent.size()
        );
        assertTrue(
            sent.get(0).body().contains(
                "\"to\":{\"kind\":\"equip\",\"slot\":\"chest\",\"state\":\"worn\"}"
            ),
            "expected body-slot payload to target CHEST worn, actual " + sent.get(0).body()
        );
    }

    @Test
    void dragEquipCommitRejectsTwoHandDisabledSlotWithoutMutationOrDispatch() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        BackpackGridPanel grid = new BackpackGridPanel("main_pack", 3, 3);
        EquipmentPanel panel = new EquipmentPanel();
        InventoryItem tool = item(2270L, "stone_pickaxe");
        grid.place(tool, 0, 1);
        panel.slotFor(EquipSlotType.OFF_HAND).setDisabledByTwoHand(true);
        screen.configureEquipInteractionForTests(grid, panel);

        boolean committed = screen.commitEquipDrop(
            tool,
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 1),
            EquipSlotType.OFF_HAND
        );

        assertFalse(committed, "expected two-hand-disabled OFF_HAND drop to be rejected, actual true");
        assertEquals(
            tool,
            grid.itemAt(0, 1),
            "expected rejected disabled-slot drop to leave the source grid item unchanged"
        );
        assertTrue(
            panel.slotFor(EquipSlotType.OFF_HAND).contents().isEmpty(),
            "expected rejected disabled-slot drop to leave OFF_HAND empty, actual non-empty"
        );
        assertTrue(
            sent.isEmpty(),
            "expected rejected disabled-slot drop to send no InventoryMoveIntent, actual " + sent.size()
        );
    }

    @Test
    void dragEquipCommitRejectsIneligibleItemWithoutMutationOrDispatch() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        BackpackGridPanel grid = new BackpackGridPanel("main_pack", 3, 3);
        EquipmentPanel panel = new EquipmentPanel();
        InventoryItem herb = item(2280L, "spirit_grass");
        grid.place(herb, 2, 2);
        screen.configureEquipInteractionForTests(grid, panel);

        boolean committed = screen.commitEquipDrop(
            herb,
            new ClientRequestProtocol.ContainerLoc("main_pack", 2, 2),
            EquipSlotType.EXTRA_HAND_0
        );

        assertFalse(committed, "expected ineligible herb drop into EXTRA_HAND_0 to be rejected, actual true");
        assertEquals(
            herb,
            grid.itemAt(2, 2),
            "expected rejected ineligible-item drop to leave the source grid item unchanged"
        );
        assertTrue(
            panel.slotFor(EquipSlotType.EXTRA_HAND_0).contents().isEmpty(),
            "expected rejected ineligible-item drop to leave EXTRA_HAND_0 empty, actual non-empty"
        );
        assertTrue(
            sent.isEmpty(),
            "expected rejected ineligible-item drop to send no InventoryMoveIntent, actual " + sent.size()
        );
    }

    @Test
    void quickUseDragToEquipRestoresSourceWhenMoveLocationCannotBeEncoded() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        EquipmentPanel panel = new EquipmentPanel();
        InventoryItem tool = item(2290L, "stone_pickaxe");
        screen.configureEquipInteractionForTests(null, panel);
        screen.beginQuickUseEquipDragForTests(tool, 2);
        sent.clear();

        boolean committed = screen.commitCurrentDragToEquipForTests(EquipSlotType.EXTRA_HAND_1);

        assertFalse(committed, "expected QUICK_USE source without InvLocation to be rejected, actual true");
        assertEquals(
            tool,
            screen.quickUseItemForTests(2),
            "expected failed QUICK_USE-to-equip move to restore the source slot item"
        );
        assertTrue(
            panel.slotFor(EquipSlotType.EXTRA_HAND_1).contents().isEmpty(),
            "expected failed QUICK_USE-to-equip move to leave EXTRA_HAND_1 unchanged, actual non-empty"
        );
        assertTrue(
            sent.stream().noneMatch(message -> message.body().contains("inventory_move_intent")),
            "expected no InventoryMoveIntent for an unencodable QUICK_USE source, actual " + sent
        );
    }

    @Test
    void dragEquipCommitRejectsMockInstanceBeforeMutatingTarget() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        EquipmentPanel panel = new EquipmentPanel();
        InventoryItem mockTool = InventoryItem.create(
            "stone_pickaxe",
            "stone_pickaxe",
            1,
            1,
            0.2,
            "common",
            "mock instance without authoritative id"
        );
        screen.configureEquipInteractionForTests(null, panel);

        boolean committed = screen.commitEquipDrop(
            mockTool,
            new ClientRequestProtocol.ContainerLoc("main_pack", 1, 1),
            EquipSlotType.EXTRA_HAND_0
        );

        assertFalse(committed, "expected instanceId=0 equip drop to be rejected, actual true");
        assertTrue(
            panel.slotFor(EquipSlotType.EXTRA_HAND_0).contents().isEmpty(),
            "expected rejected mock-instance drop to leave EXTRA_HAND_0 unchanged, actual non-empty"
        );
        assertTrue(
            sent.isEmpty(),
            "expected rejected mock-instance drop to send no request, actual " + sent.size()
        );
    }

    @Test
    void dragEquipCommitRejectsOccupiedExtraHandWithoutDispatch() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        EquipmentPanel panel = new EquipmentPanel();
        panel.slotFor(EquipSlotType.EXTRA_HAND_0)
            .setContents(SlotContents.ofHeld(item(2301L, "bone_dagger")));
        screen.configureEquipInteractionForTests(null, panel);

        boolean committed = screen.commitEquipDrop(
            item(2302L, "stone_pickaxe"),
            new ClientRequestProtocol.ContainerLoc("main_pack", 0, 0),
            EquipSlotType.EXTRA_HAND_0
        );

        assertFalse(committed, "expected occupied EXTRA_HAND_0 drag to be rejected, actual true");
        assertTrue(
            sent.isEmpty(),
            "expected rejected extra-hand drag to send no InventoryMoveIntent, actual " + sent.size()
        );
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

    private static InventoryItem item(long instanceId, String itemId) {
        return InventoryItem.createFull(
            instanceId,
            itemId,
            itemId,
            1,
            1,
            0.2,
            "common",
            "interaction fixture",
            1,
            1.0,
            1.0
        );
    }
}
