package com.bong.client.inventory;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.SlotContents;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-layered-equip-v1 P4（决议 #17/#19）：BackpackEquipSlotTest 重写。
 *
 * <p>原「背包专属槽」语义（BACK_PACK/WAIST_POUCH/CHEST_SATCHEL 枚举 + backpackWeightBreakdown 重量条）
 * 已被决议 #17/#19 取代——背包件骑身体槽 worn 层、重量沿用 BottomInfoBar。本测试改测：
 * EquipSlotType 新变体集 / InventoryModel 容器常量 + Builder / 背包件骑身体槽 SlotContents worn 层。
 */
public class BackpackEquipSlotTest {

    // ──────────────────────────────────────────────────────────────
    //  EquipSlotType 变体集（决议 #17：删 9 增 2）
    // ──────────────────────────────────────────────────────────────

    @Test
    void equipSlotTypeHasExactlyEightVariants() {
        assertEquals(8, EquipSlotType.values().length,
            "期望 8 槽（HEAD/CHEST/LEGS/FEET + MAIN/OFF/EXTRA_HAND_0/1），实际 "
                + EquipSlotType.values().length + "——检查决议 #17 删 9 增 2");
    }

    @Test
    void deletedBackpackAndTreasureVariantsAreGone() {
        for (EquipSlotType s : EquipSlotType.values()) {
            String n = s.name();
            assertFalse(n.equals("BACK_PACK") || n.equals("WAIST_POUCH") || n.equals("CHEST_SATCHEL")
                    || n.equals("FALSE_SKIN") || n.equals("TWO_HAND") || n.startsWith("TREASURE_BELT"),
                "决议 #17：废变体应已删除，发现残留 " + n);
        }
    }

    @Test
    void extraHandVariantsExist() {
        assertEquals("臂三", EquipSlotType.EXTRA_HAND_0.displayName());
        assertEquals("臂四", EquipSlotType.EXTRA_HAND_1.displayName());
        assertTrue(EquipSlotType.EXTRA_HAND_0.isHand(), "EXTRA_HAND_0 应为手槽");
        assertTrue(EquipSlotType.EXTRA_HAND_1.isHand(), "EXTRA_HAND_1 应为手槽");
    }

    @Test
    void handSlotsReportHeldAndBodySlotsReportWorn() {
        assertTrue(EquipSlotType.MAIN_HAND.isHand());
        assertTrue(EquipSlotType.OFF_HAND.isHand());
        assertFalse(EquipSlotType.HEAD.isHand());
        assertFalse(EquipSlotType.CHEST.isHand());
        assertEquals("held", EquipSlotType.MAIN_HAND.wireState());
        assertEquals("held", EquipSlotType.EXTRA_HAND_0.wireState());
        assertEquals("worn", EquipSlotType.CHEST.wireState());
        assertEquals("worn", EquipSlotType.FEET.wireState());
    }

    @Test
    void existingBodySlotDisplayNamesUnchanged() {
        assertEquals("头甲", EquipSlotType.HEAD.displayName());
        assertEquals("胸甲", EquipSlotType.CHEST.displayName());
        assertEquals("腿甲", EquipSlotType.LEGS.displayName());
        assertEquals("足甲", EquipSlotType.FEET.displayName());
        assertEquals("右手", EquipSlotType.MAIN_HAND.displayName());
        assertEquals("左手", EquipSlotType.OFF_HAND.displayName());
    }

    @Test
    void everyEquipSlotTypeHasNonBlankDisplayName() {
        for (EquipSlotType slot : EquipSlotType.values()) {
            assertNotNull(slot.displayName(), "displayName() must not be null for " + slot);
            assertTrue(!slot.displayName().isBlank(),
                "displayName() must not be blank for " + slot + ", got: '" + slot.displayName() + "'");
        }
    }

    // ──────────────────────────────────────────────────────────────
    //  InventoryModel 容器常量 + DEFAULT_CONTAINERS + Builder
    // ──────────────────────────────────────────────────────────────

    @Test
    void bodyPocketContainerIdConstantIsCorrect() {
        assertEquals("body_pocket", InventoryModel.BODY_POCKET_CONTAINER_ID);
    }

    @Test
    void backPackContainerIdConstantIsCorrect() {
        assertEquals("back_pack", InventoryModel.BACK_PACK_CONTAINER_ID);
    }

    @Test
    void defaultContainersFirstIsBodyPocket() {
        InventoryModel.ContainerDef def = InventoryModel.DEFAULT_CONTAINERS.get(0);
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, def.id());
        assertEquals(2, def.rows());
        assertEquals(3, def.cols());
    }

    @Test
    void builderWithNullContainersFallsBackToDefaultContainers() {
        InventoryModel model = InventoryModel.builder().containers(null).build();
        assertEquals(InventoryModel.DEFAULT_CONTAINERS.size(), model.containers().size());
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, model.containers().get(0).id());
    }

    @Test
    void builderWithEmptyContainersFallsBackToDefaultContainers() {
        InventoryModel model = InventoryModel.builder().containers(List.of()).build();
        assertEquals(InventoryModel.DEFAULT_CONTAINERS.size(), model.containers().size());
    }

    @Test
    void builderWithFourContainerDefsBuildsCorrectly() {
        List<InventoryModel.ContainerDef> four = List.of(
            new InventoryModel.ContainerDef("body_pocket", "贴身口袋", 2, 3),
            new InventoryModel.ContainerDef("back_pack", "背包", 3, 3),
            new InventoryModel.ContainerDef("waist_pouch", "腰包", 2, 2),
            new InventoryModel.ContainerDef("chest_satchel", "前挂", 2, 4)
        );
        InventoryModel model = InventoryModel.builder().containers(four).build();
        assertEquals(4, model.containers().size());
        assertEquals("chest_satchel", model.containers().get(3).id());
    }

    // ──────────────────────────────────────────────────────────────
    //  决议 #17：背包件骑身体槽 worn 层（不再有专属背包 EquipSlotType）
    // ──────────────────────────────────────────────────────────────

    @Test
    void backpackRidesChestWornLayerAlongsideArmor() {
        // 背包件与护甲同槽叠层：CHEST worn 栈含护甲（栈底）+ 背包件（栈顶）。
        InventoryItem armor = InventoryItem.simple("armor_bone_chestplate", "骨甲");
        InventoryItem pouch = InventoryItem.simple("worn_grass_pouch", "破草包");
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .equipSlot(EquipSlotType.CHEST, new SlotContents(List.of(armor, pouch), null))
            .build();

        SlotContents chest = model.equippedSlots().get(EquipSlotType.CHEST);
        assertNotNull(chest, "CHEST 槽应有内容");
        assertEquals(2, chest.wornCount(), "CHEST worn 应有 2 层（护甲 + 背包件）");
        assertEquals("armor_bone_chestplate", chest.worn().get(0).itemId(), "栈底应为护甲");
        assertEquals("worn_grass_pouch", chest.wornTop().itemId(), "栈顶应为背包件（决议 #12 LIFO 尾=顶）");
    }

    @Test
    void representativeOfBodySlotIsWornTop() {
        // equipped()（代表件）= worn 栈顶。
        InventoryItem armor = InventoryItem.simple("armor_bone_chestplate", "骨甲");
        InventoryItem pouch = InventoryItem.simple("worn_grass_pouch", "破草包");
        InventoryModel model = InventoryModel.builder()
            .equipSlot(EquipSlotType.CHEST, new SlotContents(List.of(armor, pouch), null))
            .build();
        assertEquals("worn_grass_pouch", model.equipped().get(EquipSlotType.CHEST).itemId(),
            "身体槽代表件应为 worn 栈顶");
    }

    @Test
    void handSlotRepresentativeIsHeld() {
        InventoryItem sword = InventoryItem.simple("iron_sword", "铁剑");
        InventoryModel model = InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, sword)
            .build();
        SlotContents main = model.equippedSlots().get(EquipSlotType.MAIN_HAND);
        assertNotNull(main);
        assertNotNull(main.held(), "手槽应为 held");
        assertEquals("iron_sword", model.equipped().get(EquipSlotType.MAIN_HAND).itemId());
        assertTrue(main.worn().isEmpty(), "手槽 worn 应为空");
    }

    @Test
    void emptySlotsAbsentFromEquippedMaps() {
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .build();
        assertNull(model.equipped().get(EquipSlotType.CHEST), "空槽不应出现在 equipped()");
        assertNull(model.equippedSlots().get(EquipSlotType.HEAD), "空槽不应出现在 equippedSlots()");
        assertTrue(model.equippedSlots().isEmpty(), "无装备时 equippedSlots() 应为空");
    }
}
