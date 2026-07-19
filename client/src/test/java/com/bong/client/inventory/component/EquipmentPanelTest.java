package com.bong.client.inventory.component;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.SlotContents;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-layered-equip-v1 P4（决议 #12/#17）：EquipmentPanel 新布局 + worn 栈渲染数据契约饱和测试。
 *
 * <p>验证：8 槽全 addSlot（中列 HEAD/CHEST/LEGS/FEET + 左右手 + 多臂，含 EXTRA_HAND_0/1）；
 * slotAtScreen 命中正确槽；populateFromModel 填分层 SlotContents；背包件骑身体槽 worn；双手武器只锁副手。
 */
class EquipmentPanelTest {

    @Test
    void allEightSlotsAreRegistered() {
        EquipmentPanel panel = new EquipmentPanel();
        assertEquals(8, panel.allSlots().size(),
            "期望 8 槽（HEAD/CHEST/LEGS/FEET + MAIN/OFF/EXTRA_HAND_0/1），实际 " + panel.allSlots().size());
        for (EquipSlotType type : EquipSlotType.values()) {
            assertNotNull(panel.slotFor(type), "决议 #17：addSlot 必须含 " + type + "（否则 drop/populate 落不进）");
        }
    }

    @Test
    void extraHandSlotsAreAddressableForDrop() {
        // major #9：extra_hand 槽必须能被 slotFor/slotAtScreen 命中，否则多臂 drop 落不进。
        EquipmentPanel panel = new EquipmentPanel();
        assertNotNull(panel.slotFor(EquipSlotType.EXTRA_HAND_0));
        assertNotNull(panel.slotFor(EquipSlotType.EXTRA_HAND_1));
    }

    @Test
    void slotsDoNotOverlap() {
        // 布局核（用 SLOT_LAYOUT 坐标表，owo 未 mount 时 x()/y() 不可读）：8 槽两两不重叠。
        int s = EquipSlotComponent.SLOT_SIZE;
        var entries = EquipmentPanel.SLOT_LAYOUT.entrySet().stream().toList();
        for (int i = 0; i < entries.size(); i++) {
            for (int j = i + 1; j < entries.size(); j++) {
                int[] a = entries.get(i).getValue();
                int[] b = entries.get(j).getValue();
                boolean overlap = a[0] < b[0] + s && b[0] < a[0] + s
                    && a[1] < b[1] + s && b[1] < a[1] + s;
                assertFalse(overlap,
                    "槽 " + entries.get(i).getKey() + "(" + a[0] + "," + a[1] + ") 与 "
                        + entries.get(j).getKey() + "(" + b[0] + "," + b[1] + ") 重叠");
            }
        }
    }

    @Test
    void allSlotsWithinPanelBounds() {
        // 布局核：每槽 (px,py) + SLOT_SIZE 落在 PANEL 内（width 140 / height 168）；worn 栈 +6px 仍内。
        int s = EquipSlotComponent.SLOT_SIZE;
        int w = EquipmentPanel.panelWidth();
        int h = EquipmentPanel.panelHeight();
        for (var e : EquipmentPanel.SLOT_LAYOUT.entrySet()) {
            int px = e.getValue()[0];
            int py = e.getValue()[1];
            assertTrue(px >= 0 && px + s <= w, e.getKey() + " x 越界 PANEL_WIDTH=" + w + ": x=" + px);
            assertTrue(py >= 0 && py + s + 6 <= h, e.getKey() + " y+栈高 越界 PANEL_HEIGHT=" + h + ": y=" + py);
        }
    }

    @Test
    void centerColumnIsVerticalLine() {
        // 中列一线：HEAD/CHEST/LEGS/FEET 同 x、y 递增。
        var L = EquipmentPanel.SLOT_LAYOUT;
        int hx = L.get(EquipSlotType.HEAD)[0];
        assertEquals(hx, L.get(EquipSlotType.CHEST)[0], "CHEST 应与 HEAD 同列");
        assertEquals(hx, L.get(EquipSlotType.LEGS)[0], "LEGS 应与 HEAD 同列");
        assertEquals(hx, L.get(EquipSlotType.FEET)[0], "FEET 应与 HEAD 同列");
        assertTrue(L.get(EquipSlotType.HEAD)[1] < L.get(EquipSlotType.CHEST)[1], "HEAD 在 CHEST 上");
        assertTrue(L.get(EquipSlotType.CHEST)[1] < L.get(EquipSlotType.LEGS)[1], "CHEST 在 LEGS 上");
        assertTrue(L.get(EquipSlotType.LEGS)[1] < L.get(EquipSlotType.FEET)[1], "LEGS 在 FEET 上");
    }

    @Test
    void handsFlankCenterColumn() {
        // 左右手对称：OFF_HAND 在中列左、MAIN_HAND 在中列右；多臂在主/副手正下方。
        var L = EquipmentPanel.SLOT_LAYOUT;
        int cx = L.get(EquipSlotType.CHEST)[0];
        assertTrue(L.get(EquipSlotType.OFF_HAND)[0] < cx, "OFF_HAND 应在中列左侧");
        assertTrue(L.get(EquipSlotType.MAIN_HAND)[0] > cx, "MAIN_HAND 应在中列右侧");
        assertEquals(L.get(EquipSlotType.OFF_HAND)[0], L.get(EquipSlotType.EXTRA_HAND_0)[0],
            "EXTRA_HAND_0 应在 OFF_HAND 正下（同列）");
        assertEquals(L.get(EquipSlotType.MAIN_HAND)[0], L.get(EquipSlotType.EXTRA_HAND_1)[0],
            "EXTRA_HAND_1 应在 MAIN_HAND 正下（同列）");
        assertTrue(L.get(EquipSlotType.OFF_HAND)[1] < L.get(EquipSlotType.EXTRA_HAND_0)[1],
            "EXTRA_HAND_0 应在 OFF_HAND 下方（y 更大）");
    }

    @Test
    void layoutTableCoversAllSlots() {
        // SLOT_LAYOUT 与 EquipSlotType 一一对应（构造器据此 addSlot）。
        assertEquals(EquipSlotType.values().length, EquipmentPanel.SLOT_LAYOUT.size(),
            "SLOT_LAYOUT 应覆盖全部 8 槽");
        for (EquipSlotType t : EquipSlotType.values()) {
            assertNotNull(EquipmentPanel.SLOT_LAYOUT.get(t), "SLOT_LAYOUT 缺槽 " + t);
        }
    }

    @Test
    void populateFillsLayeredContents() {
        // 背包件骑 CHEST worn 层（与护甲叠层）；MAIN_HAND held。
        EquipmentPanel panel = new EquipmentPanel();
        InventoryModel model = InventoryModel.builder()
            .equipSlot(EquipSlotType.CHEST, new SlotContents(List.of(
                item(1L, "armor_bone_chestplate"), item(2L, "worn_grass_pouch")), null))
            .equip(EquipSlotType.MAIN_HAND, item(3L, "iron_sword"))
            .build();
        panel.populateFromModel(model);

        EquipSlotComponent chest = panel.slotFor(EquipSlotType.CHEST);
        assertEquals(2, chest.contents().wornCount(), "CHEST 应渲染 2 worn 层");
        assertEquals("worn_grass_pouch", chest.wornTop().itemId(), "栈顶应为背包件");
        assertNotNull(panel.slotFor(EquipSlotType.MAIN_HAND).held(), "MAIN_HAND 应有 held");
    }

    @Test
    void twoHandWeaponLocksOnlyOffHand() {
        // plan-extra-hand-client-equip-gate-v1：主手持双手武器 → 只 disable 空闲副手，extra_hand 不灰显。
        EquipmentPanel panel = new EquipmentPanel();
        InventoryModel model = InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, item(1L, "wooden_staff"))
            .build();
        panel.populateFromModel(model);

        assertTrue(panel.slotFor(EquipSlotType.OFF_HAND).isDisabledByTwoHand(), "双手武器应锁副手");
        assertFalse(panel.slotFor(EquipSlotType.EXTRA_HAND_0).isDisabledByTwoHand(),
            "双手武器不应灰显多臂三；extra_hand 是独立 held 槽");
        assertFalse(panel.slotFor(EquipSlotType.EXTRA_HAND_1).isDisabledByTwoHand(),
            "双手武器不应灰显多臂四；extra_hand 是独立 held 槽");
        assertFalse(panel.slotFor(EquipSlotType.MAIN_HAND).isDisabledByTwoHand(), "主手自身不 disable");
    }

    @Test
    void singleHandWeaponDoesNotLockHands() {
        EquipmentPanel panel = new EquipmentPanel();
        InventoryModel model = InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, item(1L, "iron_sword"))
            .build();
        panel.populateFromModel(model);
        assertFalse(panel.slotFor(EquipSlotType.OFF_HAND).isDisabledByTwoHand(),
            "单手剑不应锁副手");
        assertFalse(panel.slotFor(EquipSlotType.EXTRA_HAND_0).isDisabledByTwoHand(),
            "单手剑不应锁多臂三");
        assertFalse(panel.slotFor(EquipSlotType.EXTRA_HAND_1).isDisabledByTwoHand(),
            "单手剑不应锁多臂四");
    }

    @Test
    void repopulateClearsStaleDisableAndContents() {
        // 状态转换：先双手锁，再换单手 → disable 清除、栈清空。
        EquipmentPanel panel = new EquipmentPanel();
        panel.populateFromModel(InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, item(1L, "wooden_staff")).build());
        assertTrue(panel.slotFor(EquipSlotType.OFF_HAND).isDisabledByTwoHand());

        panel.populateFromModel(InventoryModel.builder().build());
        assertFalse(panel.slotFor(EquipSlotType.OFF_HAND).isDisabledByTwoHand(),
            "重填空 model 后旧 disable 应清除");
        assertTrue(panel.slotFor(EquipSlotType.MAIN_HAND).isEmpty(), "重填后旧 held 应清空");
    }

    // ─── plan-race-system-v1 P3c：种族门置灰（槽内物品当前形态不可穿） ──────

    @org.junit.jupiter.api.AfterEach
    void tearDownRaceStores() {
        com.bong.client.inventory.state.RaceGateMetaStore.resetForTests();
        com.bong.client.inventory.state.PlayerRaceIdentityStore.resetForTests();
    }

    @Test
    void raceMismatchedEquippedItemDimsSlot() {
        // 当前形态非人形 whale，槽内是 human-only 面具 → 该槽种族门置灰。
        com.bong.client.inventory.state.PlayerRaceIdentityStore.replace(
            "human", "whale", "whale", true, false);
        com.bong.client.inventory.state.RaceGateMetaStore.replace(
            java.util.Map.of("human_mask", new com.bong.client.inventory.model.RaceGate("humanoid", List.of())),
            java.util.Map.of());

        EquipmentPanel panel = new EquipmentPanel();
        panel.populateFromModel(InventoryModel.builder()
            .equip(EquipSlotType.HEAD, item(1L, "human_mask")).build());

        assertTrue(panel.slotFor(EquipSlotType.HEAD).isDisabledByRace(),
            "当前形态非人形时 human-only 装备槽应种族门置灰");
        assertTrue(panel.slotFor(EquipSlotType.HEAD).isInteractionBlocked(),
            "种族置灰的槽应整体不可交互（禁拖入/拖出）");
    }

    @Test
    void raceMatchedEquippedItemDoesNotDim() {
        com.bong.client.inventory.state.PlayerRaceIdentityStore.replace(
            "human", "human", "humanoid", true, true);
        com.bong.client.inventory.state.RaceGateMetaStore.replace(
            java.util.Map.of("human_mask", new com.bong.client.inventory.model.RaceGate("humanoid", List.of())),
            java.util.Map.of());

        EquipmentPanel panel = new EquipmentPanel();
        panel.populateFromModel(InventoryModel.builder()
            .equip(EquipSlotType.HEAD, item(1L, "human_mask")).build());

        assertFalse(panel.slotFor(EquipSlotType.HEAD).isDisabledByRace(),
            "未易形（人形形态）时 human-only 装备不应置灰");
    }

    @Test
    void itemWithoutGateNeverDims() {
        com.bong.client.inventory.state.PlayerRaceIdentityStore.replace(
            "human", "whale", "whale", true, false);
        // 表内无该 item → any → 不置灰（即便当前形态非人形）。
        com.bong.client.inventory.state.RaceGateMetaStore.replace(java.util.Map.of(), java.util.Map.of());

        EquipmentPanel panel = new EquipmentPanel();
        panel.populateFromModel(InventoryModel.builder()
            .equip(EquipSlotType.HEAD, item(1L, "plain_helmet")).build());

        assertFalse(panel.slotFor(EquipSlotType.HEAD).isDisabledByRace(),
            "无 wearer_race 门的物品（any）永不因种族置灰");
    }

    @Test
    void emptySlotNeverDimsByRace() {
        com.bong.client.inventory.state.PlayerRaceIdentityStore.replace(
            "human", "whale", "whale", true, false);
        com.bong.client.inventory.state.RaceGateMetaStore.replace(
            java.util.Map.of("human_mask", new com.bong.client.inventory.model.RaceGate("humanoid", List.of())),
            java.util.Map.of());

        EquipmentPanel panel = new EquipmentPanel();
        panel.populateFromModel(InventoryModel.builder().build());

        assertFalse(panel.slotFor(EquipSlotType.HEAD).isDisabledByRace(),
            "空槽无物品可判，绝不因种族置灰");
    }

    @Test
    void repopulateClearsStaleRaceDim() {
        // 状态转换：先易形成 whale 令 human 面具置灰，再变回人形 → 置灰清除。
        com.bong.client.inventory.state.RaceGateMetaStore.replace(
            java.util.Map.of("human_mask", new com.bong.client.inventory.model.RaceGate("humanoid", List.of())),
            java.util.Map.of());
        com.bong.client.inventory.state.PlayerRaceIdentityStore.replace(
            "human", "whale", "whale", true, false);
        EquipmentPanel panel = new EquipmentPanel();
        panel.populateFromModel(InventoryModel.builder()
            .equip(EquipSlotType.HEAD, item(1L, "human_mask")).build());
        assertTrue(panel.slotFor(EquipSlotType.HEAD).isDisabledByRace());

        // 变回人形形态后 repopulate。
        com.bong.client.inventory.state.PlayerRaceIdentityStore.replace(
            "human", "human", "humanoid", true, true);
        panel.populateFromModel(InventoryModel.builder()
            .equip(EquipSlotType.HEAD, item(1L, "human_mask")).build());
        assertFalse(panel.slotFor(EquipSlotType.HEAD).isDisabledByRace(),
            "变回人形后 repopulate 应清除旧种族置灰");
    }

    @Test
    void failClosedDimsWhenFormIdentityUnknown() {
        // 收到 meta 但身份快照未到（form_race_id 空）→ 有 gate 的装备置灰。
        com.bong.client.inventory.state.RaceGateMetaStore.replace(
            java.util.Map.of("human_mask", new com.bong.client.inventory.model.RaceGate("humanoid", List.of())),
            java.util.Map.of());
        // 不调用 PlayerRaceIdentityStore.replace → 身份未知。

        EquipmentPanel panel = new EquipmentPanel();
        panel.populateFromModel(InventoryModel.builder()
            .equip(EquipSlotType.HEAD, item(1L, "human_mask")).build());

        assertTrue(panel.slotFor(EquipSlotType.HEAD).isDisabledByRace(),
            "身份未知（首帧乱序）时有 gate 的装备 fail-closed 置灰");
    }

    private static InventoryItem item(long id, String itemId) {
        return InventoryItem.createFull(id, itemId, itemId, 1, 1, 1.0, "common", "", 1, 1.0, 1.0);
    }
}
