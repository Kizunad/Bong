package com.bong.client.armor;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.SlotContents;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ArmorFeatureRendererTest {
    @AfterEach
    void clearDevProperty() {
        System.clearProperty(ArmorFeatureRenderer.DEV_RENDER_PROPERTY);
    }

    @Test
    void p0FormalSwitchIsOffButDevPropertyCanExercisePipeline() {
        assertFalse(ArmorFeatureRenderer.OBJ_RENDER_READY, "P3 前正式开关必须保持关闭");
        assertFalse(ArmorFeatureRenderer.isModelRenderEnabled());
        System.setProperty(ArmorFeatureRenderer.DEV_RENDER_PROPERTY, "true");
        assertTrue(ArmorFeatureRenderer.isModelRenderEnabled(), "dev property 应允许 P1/P2 增量真机核验");
        System.setProperty(ArmorFeatureRenderer.DEV_RENDER_PROPERTY, "false");
        assertFalse(ArmorFeatureRenderer.isModelRenderEnabled());
    }

    @Test
    void collectRenderableRejectsNullEmptyUnknownWrongSlotAndBrokenItems() {
        assertTrue(ArmorFeatureRenderer.collectRenderable(null).isEmpty());
        assertTrue(ArmorFeatureRenderer.collectRenderable(Map.of()).isEmpty());

        EnumMap<EquipSlotType, SlotContents> slots = new EnumMap<>(EquipSlotType.class);
        slots.put(EquipSlotType.HEAD, new SlotContents(List.of(
            item("unknown", 1.0),
            item("armor_iron_chestplate", 1.0),
            item("armor_iron_helmet", 0.0)
        ), null));
        assertTrue(ArmorFeatureRenderer.collectRenderable(slots).isEmpty(),
            "未知、错槽、耐久 0 三条错误分支都不得进入渲染表");
    }

    @Test
    void collectRenderableKeepsPositiveBoundaryAndArmorBelowLayeredPack() {
        EnumMap<EquipSlotType, SlotContents> slots = new EnumMap<>(EquipSlotType.class);
        slots.put(EquipSlotType.CHEST, new SlotContents(List.of(
            item("armor_iron_chestplate", Double.MIN_VALUE),
            item("grass_pouch", 1.0)
        ), null));

        List<ArmorFeatureRenderer.RenderableArmor> result = ArmorFeatureRenderer.collectRenderable(slots);
        assertEquals(1, result.size(), "worn 栈上层背包不得遮掉下层胸甲模型");
        assertEquals(EquipSlotType.CHEST, result.get(0).slot());
        assertEquals("iron_chestplate", result.get(0).spec().modelKey());
    }

    @Test
    void collectRenderableCoversTwoMaterialsAcrossAllFourSlots() {
        for (String material : new String[]{"iron", "bone"}) {
            EnumMap<EquipSlotType, SlotContents> slots = new EnumMap<>(EquipSlotType.class);
            slots.put(EquipSlotType.HEAD, SlotContents.ofWorn(item("armor_" + material + "_helmet", 1.0)));
            slots.put(EquipSlotType.CHEST, SlotContents.ofWorn(item("armor_" + material + "_chestplate", 1.0)));
            slots.put(EquipSlotType.LEGS, SlotContents.ofWorn(item("armor_" + material + "_leggings", 1.0)));
            slots.put(EquipSlotType.FEET, SlotContents.ofWorn(item("armor_" + material + "_boots", 1.0)));

            List<ArmorFeatureRenderer.RenderableArmor> result = ArmorFeatureRenderer.collectRenderable(slots);
            assertEquals(4, result.size(), material + " 四部位都应进入渲染表");
            assertEquals(ArmorFeatureRenderer.ARMOR_SLOTS, result.stream().map(
                ArmorFeatureRenderer.RenderableArmor::slot).toList());
        }
    }

    private static InventoryItem item(String itemId, double durability) {
        return InventoryItem.createFull(
            1L,
            itemId,
            itemId,
            1,
            1,
            1.0,
            "common",
            "",
            1,
            1.0,
            durability
        );
    }
}
