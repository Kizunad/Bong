package com.bong.client.inventory;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * P4.1 / P4.2 / P4.5 tests for plan-backpack-equip-v1.
 *
 * Covers:
 *  - EquipSlotType new variants (BACK_PACK, WAIST_POUCH, CHEST_SATCHEL)
 *  - InventoryModel new constants (BODY_POCKET_CONTAINER_ID, BACK_PACK_CONTAINER_ID)
 *  - DEFAULT_CONTAINERS new layout (body_pocket 2×3 + back_pack 3×3)
 *  - Builder.containers() with 0–4 ContainerDef entries
 *  - 行囊 tab weight breakdown label via InspectScreen.backpackWeightBreakdown()
 */
public class BackpackEquipSlotTest {

    // ──────────────────────────────────────────────────────────────
    //  P4.1 — EquipSlotType new variants
    // ──────────────────────────────────────────────────────────────

    @Test
    void backPackVariantExistsWithCorrectDisplayName() {
        EquipSlotType slot = EquipSlotType.BACK_PACK;
        assertEquals("背包", slot.displayName(),
            "BACK_PACK displayName should be '背包', got: " + slot.displayName());
    }

    @Test
    void waistPouchVariantExistsWithCorrectDisplayName() {
        EquipSlotType slot = EquipSlotType.WAIST_POUCH;
        assertEquals("腰包", slot.displayName(),
            "WAIST_POUCH displayName should be '腰包', got: " + slot.displayName());
    }

    @Test
    void chestSatchelVariantExistsWithCorrectDisplayName() {
        EquipSlotType slot = EquipSlotType.CHEST_SATCHEL;
        assertEquals("前挂", slot.displayName(),
            "CHEST_SATCHEL displayName should be '前挂', got: " + slot.displayName());
    }

    @Test
    void allThreeNewBackpackSlotsAreDistinct() {
        EquipSlotType back = EquipSlotType.BACK_PACK;
        EquipSlotType waist = EquipSlotType.WAIST_POUCH;
        EquipSlotType chest = EquipSlotType.CHEST_SATCHEL;
        assertTrue(back != waist, "BACK_PACK and WAIST_POUCH must be distinct variants");
        assertTrue(waist != chest, "WAIST_POUCH and CHEST_SATCHEL must be distinct variants");
        assertTrue(back != chest, "BACK_PACK and CHEST_SATCHEL must be distinct variants");
    }

    @Test
    void everyEquipSlotTypeHasNonBlankDisplayName() {
        for (EquipSlotType slot : EquipSlotType.values()) {
            assertNotNull(slot.displayName(), "displayName() must not be null for " + slot);
            assertTrue(!slot.displayName().isBlank(),
                "displayName() must not be blank for " + slot + ", got: '" + slot.displayName() + "'");
        }
    }

    @Test
    void existingSlotTypesAreUnchanged() {
        // Regression guard: pre-existing slots must retain their display names.
        assertEquals("头甲", EquipSlotType.HEAD.displayName());
        assertEquals("胸甲", EquipSlotType.CHEST.displayName());
        assertEquals("腿甲", EquipSlotType.LEGS.displayName());
        assertEquals("足甲", EquipSlotType.FEET.displayName());
        assertEquals("伪皮", EquipSlotType.FALSE_SKIN.displayName());
        assertEquals("右手", EquipSlotType.MAIN_HAND.displayName());
        assertEquals("左手", EquipSlotType.OFF_HAND.displayName());
        assertEquals("双手", EquipSlotType.TWO_HAND.displayName());
        assertEquals("宝1", EquipSlotType.TREASURE_BELT_0.displayName());
        assertEquals("宝2", EquipSlotType.TREASURE_BELT_1.displayName());
        assertEquals("宝3", EquipSlotType.TREASURE_BELT_2.displayName());
        assertEquals("宝4", EquipSlotType.TREASURE_BELT_3.displayName());
    }

    // ──────────────────────────────────────────────────────────────
    //  P4.2 — InventoryModel constants + DEFAULT_CONTAINERS
    // ──────────────────────────────────────────────────────────────

    @Test
    void bodyPocketContainerIdConstantIsCorrect() {
        assertEquals("body_pocket", InventoryModel.BODY_POCKET_CONTAINER_ID,
            "BODY_POCKET_CONTAINER_ID must be 'body_pocket'");
    }

    @Test
    void backPackContainerIdConstantIsCorrect() {
        assertEquals("back_pack", InventoryModel.BACK_PACK_CONTAINER_ID,
            "BACK_PACK_CONTAINER_ID must be 'back_pack'");
    }

    @Test
    void defaultContainersHasTwoEntries() {
        assertEquals(2, InventoryModel.DEFAULT_CONTAINERS.size(),
            "DEFAULT_CONTAINERS should have exactly 2 entries: body_pocket + back_pack, size was: "
                + InventoryModel.DEFAULT_CONTAINERS.size());
    }

    @Test
    void defaultContainersFirstIsBodyPocket() {
        InventoryModel.ContainerDef def = InventoryModel.DEFAULT_CONTAINERS.get(0);
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, def.id(),
            "First DEFAULT_CONTAINERS entry should be body_pocket, got: " + def.id());
        assertEquals(2, def.rows(),
            "body_pocket should be 2 rows, got: " + def.rows());
        assertEquals(3, def.cols(),
            "body_pocket should be 3 cols, got: " + def.cols());
    }

    @Test
    void defaultContainersSecondIsBackPack() {
        InventoryModel.ContainerDef def = InventoryModel.DEFAULT_CONTAINERS.get(1);
        assertEquals(InventoryModel.BACK_PACK_CONTAINER_ID, def.id(),
            "Second DEFAULT_CONTAINERS entry should be back_pack, got: " + def.id());
        assertEquals(3, def.rows(),
            "back_pack should be 3 rows, got: " + def.rows());
        assertEquals(3, def.cols(),
            "back_pack should be 3 cols, got: " + def.cols());
    }

    // ──────────────────────────────────────────────────────────────
    //  P4.2 — Builder.containers() with varying ContainerDef counts
    // ──────────────────────────────────────────────────────────────

    @Test
    void builderWithNullContainersFallsBackToDefaultContainers() {
        InventoryModel model = InventoryModel.builder().containers(null).build();
        assertEquals(InventoryModel.DEFAULT_CONTAINERS.size(), model.containers().size(),
            "null containers should fall back to DEFAULT_CONTAINERS");
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, model.containers().get(0).id());
    }

    @Test
    void builderWithEmptyContainersFallsBackToDefaultContainers() {
        InventoryModel model = InventoryModel.builder().containers(List.of()).build();
        assertEquals(InventoryModel.DEFAULT_CONTAINERS.size(), model.containers().size(),
            "empty containers should fall back to DEFAULT_CONTAINERS");
    }

    @Test
    void builderWithOneContainerDefBuildsSingleContainerModel() {
        List<InventoryModel.ContainerDef> one = List.of(
            new InventoryModel.ContainerDef(InventoryModel.BODY_POCKET_CONTAINER_ID, "贴身口袋", 2, 3)
        );
        InventoryModel model = InventoryModel.builder().containers(one).build();
        assertEquals(1, model.containers().size(),
            "Builder with 1 ContainerDef should produce model with 1 container");
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, model.containers().get(0).id());
    }

    @Test
    void builderWithThreeContainerDefsBuildsCorrectly() {
        List<InventoryModel.ContainerDef> three = List.of(
            new InventoryModel.ContainerDef(InventoryModel.BODY_POCKET_CONTAINER_ID, "贴身口袋", 2, 3),
            new InventoryModel.ContainerDef(InventoryModel.BACK_PACK_CONTAINER_ID, "草包", 3, 3),
            new InventoryModel.ContainerDef("waist_pouch", "腰包", 2, 2)
        );
        InventoryModel model = InventoryModel.builder().containers(three).build();
        assertEquals(3, model.containers().size(),
            "Builder with 3 ContainerDefs should produce model with 3 containers");
        assertEquals("waist_pouch", model.containers().get(2).id());
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
        assertEquals(4, model.containers().size(),
            "Builder with 4 ContainerDefs should produce model with 4 containers");
        assertEquals("chest_satchel", model.containers().get(3).id());
    }

    @Test
    void builderGridItemDefaultsToPrimaryContainerOfFirstContainerDef() {
        // Verify gridItem(item, row, col) routes to first container when containers() is set.
        List<InventoryModel.ContainerDef> defs = List.of(
            new InventoryModel.ContainerDef("body_pocket", "贴身口袋", 2, 3),
            new InventoryModel.ContainerDef("back_pack", "背包", 3, 3)
        );
        InventoryModel model = InventoryModel.builder()
            .containers(defs)
            .gridItem(InventoryItem.simple("test_item", "测试"), 0, 0)
            .build();
        assertEquals("body_pocket", model.gridItems().get(0).containerId(),
            "gridItem without explicit container should route to first container def");
    }

    // ──────────────────────────────────────────────────────────────
    //  P4.3 — 行囊 tab weight breakdown logic
    // ──────────────────────────────────────────────────────────────

    @Test
    void weightBreakdownNormalLoad() {
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .weight(12.3, 23.0)
            .build();
        String result = InspectScreen.backpackWeightBreakdown(model);
        assertTrue(result.contains("12.3"),
            "breakdown should contain current weight 12.3, got: " + result);
        assertTrue(result.contains("23.0"),
            "breakdown should contain max weight 23.0, got: " + result);
        assertTrue(!result.contains("负重过载"),
            "breakdown should NOT show overload text when within limit, got: " + result);
    }

    @Test
    void weightBreakdownOverload() {
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .weight(30.0, 23.0)
            .build();
        String result = InspectScreen.backpackWeightBreakdown(model);
        assertTrue(result.contains("30.0"),
            "breakdown should contain current weight 30.0, got: " + result);
        assertTrue(result.contains("负重过载"),
            "breakdown should show '负重过载' when overweight, got: " + result);
    }

    @Test
    void weightBreakdownExactlyAtLimit() {
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .weight(23.0, 23.0)
            .build();
        String result = InspectScreen.backpackWeightBreakdown(model);
        // Exactly at max is NOT overload (currentWeight > maxWeight is false when equal)
        assertTrue(!result.contains("负重过载"),
            "breakdown at exact limit should NOT show overload, got: " + result);
    }

    @Test
    void weightBreakdownNullModelReturnsEmpty() {
        String result = InspectScreen.backpackWeightBreakdown(null);
        assertEquals("", result,
            "breakdown with null model should return empty string");
    }

    @Test
    void weightBreakdownZeroWeight() {
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .weight(0.0, 15.0)
            .build();
        String result = InspectScreen.backpackWeightBreakdown(model);
        assertTrue(result.contains("0.0"),
            "breakdown should contain 0.0 current weight, got: " + result);
        assertTrue(result.contains("15.0"),
            "breakdown should contain 15.0 max weight, got: " + result);
    }

    // ──────────────────────────────────────────────────────────────
    //  P4.2 — InventoryModel.equipped() supports new backpack slots
    // ──────────────────────────────────────────────────────────────

    @Test
    void modelEquippedSupportsBackPackSlot() {
        InventoryItem pouch = InventoryItem.simple("worn_grass_pouch", "破草包");
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .equip(EquipSlotType.BACK_PACK, pouch)
            .build();
        assertNotNull(model.equipped().get(EquipSlotType.BACK_PACK),
            "equipped() should contain BACK_PACK item");
        assertEquals("worn_grass_pouch", model.equipped().get(EquipSlotType.BACK_PACK).itemId());
    }

    @Test
    void modelEquippedSupportsWaistPouchSlot() {
        InventoryItem pouch = InventoryItem.simple("small_waist_bag", "小腰袋");
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .equip(EquipSlotType.WAIST_POUCH, pouch)
            .build();
        assertNotNull(model.equipped().get(EquipSlotType.WAIST_POUCH),
            "equipped() should contain WAIST_POUCH item");
        assertEquals("small_waist_bag", model.equipped().get(EquipSlotType.WAIST_POUCH).itemId());
    }

    @Test
    void modelEquippedSupportsChestSatchelSlot() {
        InventoryItem satchel = InventoryItem.simple("chest_satchel_item", "前挂包");
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .equip(EquipSlotType.CHEST_SATCHEL, satchel)
            .build();
        assertNotNull(model.equipped().get(EquipSlotType.CHEST_SATCHEL),
            "equipped() should contain CHEST_SATCHEL item");
        assertEquals("chest_satchel_item", model.equipped().get(EquipSlotType.CHEST_SATCHEL).itemId());
    }

    @Test
    void modelEquippedAllThreeBackpackSlotsSimultaneously() {
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .equip(EquipSlotType.BACK_PACK, InventoryItem.simple("worn_grass_pouch", "破草包"))
            .equip(EquipSlotType.WAIST_POUCH, InventoryItem.simple("small_waist_bag", "小腰袋"))
            .equip(EquipSlotType.CHEST_SATCHEL, InventoryItem.simple("chest_satchel_item", "前挂包"))
            .build();
        assertNotNull(model.equipped().get(EquipSlotType.BACK_PACK));
        assertNotNull(model.equipped().get(EquipSlotType.WAIST_POUCH));
        assertNotNull(model.equipped().get(EquipSlotType.CHEST_SATCHEL));
        assertEquals("worn_grass_pouch", model.equipped().get(EquipSlotType.BACK_PACK).itemId());
        assertEquals("small_waist_bag", model.equipped().get(EquipSlotType.WAIST_POUCH).itemId());
        assertEquals("chest_satchel_item", model.equipped().get(EquipSlotType.CHEST_SATCHEL).itemId());
    }

    @Test
    void modelEquippedBackpackSlotsAreEmptyByDefault() {
        InventoryModel model = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .build();
        assertNull(model.equipped().get(EquipSlotType.BACK_PACK),
            "BACK_PACK slot should be null when not equipped");
        assertNull(model.equipped().get(EquipSlotType.WAIST_POUCH),
            "WAIST_POUCH slot should be null when not equipped");
        assertNull(model.equipped().get(EquipSlotType.CHEST_SATCHEL),
            "CHEST_SATCHEL slot should be null when not equipped");
    }
}
