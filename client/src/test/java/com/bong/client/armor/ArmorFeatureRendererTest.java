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
import static org.junit.jupiter.api.Assertions.assertTrue;

class ArmorFeatureRendererTest {
    @AfterEach
    void clearDevProperty() {
        System.clearProperty(ArmorFeatureRenderer.DEV_RENDER_PROPERTY);
    }

    @Test
    void p3FormalSwitchIsOnAndDevPropertyCannotDisableIt() {
        assertTrue(ArmorFeatureRenderer.MODEL_RENDER_READY, "P3 后正式 ModelPart 开关必须开启");
        assertTrue(ArmorFeatureRenderer.isModelRenderEnabled());
        System.setProperty(ArmorFeatureRenderer.DEV_RENDER_PROPERTY, "true");
        assertTrue(ArmorFeatureRenderer.isModelRenderEnabled());
        System.setProperty(ArmorFeatureRenderer.DEV_RENDER_PROPERTY, "false");
        assertTrue(ArmorFeatureRenderer.isModelRenderEnabled(), "dev property=false 不得成为正式渲染 kill switch");
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

    @Test
    void collectRenderableCoversTwoMaterialsFourSlotsAndWearRemoveBrokenStates() {
        EnumMap<EquipSlotType, String> pieces = new EnumMap<>(EquipSlotType.class);
        pieces.put(EquipSlotType.HEAD, "helmet");
        pieces.put(EquipSlotType.CHEST, "chestplate");
        pieces.put(EquipSlotType.LEGS, "leggings");
        pieces.put(EquipSlotType.FEET, "boots");

        for (String material : new String[]{"iron", "bone"}) {
            for (Map.Entry<EquipSlotType, String> entry : pieces.entrySet()) {
                String templateId = "armor_" + material + "_" + entry.getValue();
                EnumMap<EquipSlotType, SlotContents> worn = new EnumMap<>(EquipSlotType.class);
                worn.put(entry.getKey(), SlotContents.ofWorn(item(templateId, 1.0)));
                assertEquals(1, ArmorFeatureRenderer.collectRenderable(worn).size(),
                    templateId + " 穿戴态应渲染 1 件");

                EnumMap<EquipSlotType, SlotContents> removed = new EnumMap<>(EquipSlotType.class);
                removed.put(entry.getKey(), SlotContents.empty());
                assertTrue(ArmorFeatureRenderer.collectRenderable(removed).isEmpty(),
                    templateId + " 脱下态不得渲染");

                EnumMap<EquipSlotType, SlotContents> broken = new EnumMap<>(EquipSlotType.class);
                broken.put(entry.getKey(), SlotContents.ofWorn(item(templateId, 0.0)));
                assertTrue(ArmorFeatureRenderer.collectRenderable(broken).isEmpty(),
                    templateId + " 耐久归零态不得渲染");
            }
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
