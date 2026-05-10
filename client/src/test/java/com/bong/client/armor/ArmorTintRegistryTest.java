package com.bong.client.armor;

import net.minecraft.entity.EquipmentSlot;
import org.junit.jupiter.api.Test;

import java.util.HashSet;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ArmorTintRegistryTest {
    @Test
    void boneArmorTintMatchesPlanColor() {
        assertEquals(0xD0C8B8, ArmorTintRegistry.tintForItemId("armor_bone_chestplate"));
        assertEquals(0xFFD0C8B8, ArmorTintRegistry.argbForItemIdOrDefault("armor_bone_chestplate", 0));
    }

    @Test
    void allSixMaterialsHaveDistinctColorsAndTwentyFourItems() {
        assertEquals(6, ArmorTintRegistry.materialCount());
        assertEquals(24, ArmorTintRegistry.itemCount());

        Set<Integer> colors = new HashSet<>();
        for (String itemId : java.util.List.of(
            "armor_bone_chestplate",
            "armor_hide_chestplate",
            "armor_iron_chestplate",
            "armor_copper_chestplate",
            "armor_spirit_cloth_chestplate",
            "armor_scroll_wrap_chestplate"
        )) {
            colors.add(ArmorTintRegistry.tintForItemId(itemId));
        }
        assertEquals(6, colors.size(), "6 套凡物甲必须有 6 个可区分 tint");
    }

    @Test
    void itemSpecsExposeSlotIconAndTooltipLines() {
        ArmorTintRegistry.ArmorItemSpec helmet = ArmorTintRegistry.item("Armor_Spirit_Cloth_Helmet");

        assertNotNull(helmet);
        assertEquals(EquipmentSlot.HEAD, helmet.slot());
        assertEquals("凡物·灵布", ArmorTintRegistry.materialLine("armor_spirit_cloth_helmet"));
        assertEquals("防御: +0.60", ArmorTintRegistry.defenseLine("armor_spirit_cloth_helmet"));
        assertEquals(
            "bong-client:textures/gui/items/armor/armor_spirit_cloth.png",
            ArmorTintRegistry.iconPathForItemId("armor_spirit_cloth_boots")
        );
        assertTrue(ArmorTintRegistry.isMundaneArmor("armor_scroll_wrap_leggings"));
        assertNull(ArmorTintRegistry.item("fake_spirit_hide"));
    }
}
