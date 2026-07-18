package com.bong.client.armor;

import com.bong.client.inventory.model.EquipSlotType;
import net.minecraft.client.model.ModelPart;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ArmorPartModelTest {
    @Test
    void eachArmorSlotUsesItsOwnBonePivotConversion() {
        assertEquals(-8.0f, ArmorPartModel.bedrockToVanillaCuboidY(EquipSlotType.HEAD, 24.0f, 8.0f), 1e-4f,
            "head y=24..32 应转成 vanilla -8..0");
        assertEquals(0.0f, ArmorPartModel.bedrockToVanillaCuboidY(EquipSlotType.CHEST, 12.0f, 12.0f), 1e-4f,
            "chest y=12..24 应转成 vanilla 0..12");
        assertEquals(0.0f, ArmorPartModel.bedrockToVanillaCuboidY(EquipSlotType.LEGS, 0.0f, 12.0f), 1e-4f,
            "legs y=0..12 应转成 vanilla 0..12");
        assertEquals(8.0f, ArmorPartModel.bedrockToVanillaCuboidY(EquipSlotType.FEET, 0.0f, 4.0f), 1e-4f,
            "feet y=0..4 应落在腿骨末端 vanilla 8..12");
    }

    @Test
    void coordinateConversionRejectsNonArmorAndNullSlots() {
        assertThrows(IllegalArgumentException.class,
            () -> ArmorPartModel.bedrockToVanillaCuboidY(EquipSlotType.MAIN_HAND, 0.0f, 1.0f));
        assertThrows(IllegalArgumentException.class,
            () -> ArmorPartModel.bedrockToVanillaCuboidY(null, 0.0f, 1.0f));
    }

    @Test
    void fourSlotsMapToCorrectPlayerBones() {
        assertEquals(List.of(ArmorPartModel.Mount.HEAD), ArmorPartModel.mountsForSlot(EquipSlotType.HEAD));
        assertEquals(List.of(ArmorPartModel.Mount.BODY), ArmorPartModel.mountsForSlot(EquipSlotType.CHEST));
        assertEquals(List.of(ArmorPartModel.Mount.LEFT_LEG, ArmorPartModel.Mount.RIGHT_LEG),
            ArmorPartModel.mountsForSlot(EquipSlotType.LEGS));
        assertEquals(List.of(ArmorPartModel.Mount.LEFT_FOOT, ArmorPartModel.Mount.RIGHT_FOOT),
            ArmorPartModel.mountsForSlot(EquipSlotType.FEET));
        assertTrue(ArmorPartModel.mountsForSlot(EquipSlotType.MAIN_HAND).isEmpty());
        assertTrue(ArmorPartModel.mountsForSlot(null).isEmpty());
    }

    @Test
    void everyCubeTableIsPinnedAndBakesExpectedChildren() {
        assertPinnedTable("iron_helmet", 15, ArmorPartModel.Mount.HEAD, -4.4f, 31.4f,
            ArmorPartModel.Mount.HEAD, -3.8f, 23.7f);
        assertPinnedTable("iron_chestplate", 23, ArmorPartModel.Mount.BODY, -4.25f, 18.0f,
            ArmorPartModel.Mount.BODY, 2.2f, 15.2f);
        assertPinnedTable("iron_leggings", 18, ArmorPartModel.Mount.LEFT_LEG, -2.05f, 7.0f,
            ArmorPartModel.Mount.RIGHT_LEG, -2.3f, 3.4f);
        assertPinnedTable("iron_boots", 14, ArmorPartModel.Mount.LEFT_FOOT, -2.1f, 2.0f,
            ArmorPartModel.Mount.RIGHT_FOOT, -2.35f, -0.25f);
        assertPinnedSingle("bone_helmet", ArmorPartModel.Mount.HEAD, -4.5f, 23.7f, 9.0f, 8.8f);
        assertPinnedSingle("bone_chestplate", ArmorPartModel.Mount.BODY, -4.5f, 11.7f, 9.0f, 12.6f);
        assertPinnedPair("bone_leggings", ArmorPartModel.Mount.LEFT_LEG, ArmorPartModel.Mount.RIGHT_LEG, 12.4f);
        assertPinnedPair("bone_boots", ArmorPartModel.Mount.LEFT_FOOT, ArmorPartModel.Mount.RIGHT_FOOT, 4.4f);
    }

    private static void assertPinnedTable(
        String modelKey,
        int expectedCount,
        ArmorPartModel.Mount firstMount,
        float firstOx,
        float firstOy,
        ArmorPartModel.Mount lastMount,
        float lastOx,
        float lastOy
    ) {
        List<ArmorPartModel.ArmorCube> cubes = ArmorPartModel.cubes(modelKey);
        assertEquals(expectedCount, cubes.size(), modelKey + " cube 数量漂移");
        ArmorPartModel.ArmorCube first = cubes.get(0);
        ArmorPartModel.ArmorCube last = cubes.get(cubes.size() - 1);
        assertEquals(firstMount, first.mount());
        assertEquals(firstOx, first.ox(), 1e-4f);
        assertEquals(firstOy, first.oy(), 1e-4f);
        assertEquals(lastMount, last.mount());
        assertEquals(lastOx, last.ox(), 1e-4f);
        assertEquals(lastOy, last.oy(), 1e-4f);
        ModelPart root = ArmorPartModel.buildModelPart(modelKey);
        for (ArmorPartModel.Mount mount : cubes.stream().map(ArmorPartModel.ArmorCube::mount).distinct().toList()) {
            assertNotNull(root.getChild(mount.childName()), modelKey + " 缺烘焙 child " + mount);
        }
    }

    @Test
    void supportAndBuildRejectUnknownKeys() {
        assertFalse(ArmorPartModel.supports(null));
        assertFalse(ArmorPartModel.supports("missing"));
        assertThrows(IllegalArgumentException.class, () -> ArmorPartModel.buildModelPart("missing"));
    }

    private static void assertPinnedSingle(
        String modelKey,
        ArmorPartModel.Mount mount,
        float ox,
        float oy,
        float sx,
        float sy
    ) {
        List<ArmorPartModel.ArmorCube> cubes = ArmorPartModel.cubes(modelKey);
        assertEquals(1, cubes.size(), modelKey + " P0 占位表应恰有一个 cube");
        ArmorPartModel.ArmorCube cube = cubes.get(0);
        assertEquals(mount, cube.mount());
        assertEquals(ox, cube.ox(), 1e-4f);
        assertEquals(oy, cube.oy(), 1e-4f);
        assertEquals(sx, cube.sx(), 1e-4f);
        assertEquals(sy, cube.sy(), 1e-4f);
        ModelPart root = ArmorPartModel.buildModelPart(modelKey);
        assertNotNull(root.getChild(mount.childName()));
    }

    private static void assertPinnedPair(
        String modelKey,
        ArmorPartModel.Mount left,
        ArmorPartModel.Mount right,
        float expectedHeight
    ) {
        List<ArmorPartModel.ArmorCube> cubes = ArmorPartModel.cubes(modelKey);
        assertEquals(2, cubes.size(), modelKey + " 应拆为左右两个独立骨骼 cube");
        assertEquals(left, cubes.get(0).mount());
        assertEquals(right, cubes.get(1).mount());
        assertEquals(expectedHeight, cubes.get(0).sy(), 1e-4f);
        ModelPart root = ArmorPartModel.buildModelPart(modelKey);
        assertNotNull(root.getChild(left.childName()));
        assertNotNull(root.getChild(right.childName()));
    }
}
