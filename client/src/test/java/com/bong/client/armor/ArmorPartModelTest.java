package com.bong.client.armor;

import com.bong.client.inventory.model.EquipSlotType;
import net.minecraft.client.model.ModelPart;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;

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
        assertPinnedTable("bone_helmet", 21, ArmorPartModel.Mount.HEAD, -3.7f, 28.7f,
            ArmorPartModel.Mount.HEAD, -4.75f, 27.0f);
        assertPinnedTable("bone_chestplate", 37, ArmorPartModel.Mount.BODY, -0.7f, 19.0f,
            ArmorPartModel.Mount.BODY, 3.65f, 12.25f);
        assertPinnedTable("bone_leggings", 30, ArmorPartModel.Mount.LEFT_LEG, -0.5f, 5.1f,
            ArmorPartModel.Mount.RIGHT_LEG, -1.7f, 3.5f);
        assertPinnedTable("bone_boots", 28, ArmorPartModel.Mount.LEFT_FOOT, -0.55f, 1.8f,
            ArmorPartModel.Mount.RIGHT_FOOT, 1.75f, 2.75f);
    }

    @Test
    void everyCubeFieldIsPinnedByStableDigest() {
        Map<String, String> expected = Map.of(
            "iron_helmet", "3760e3b372a70fda",
            "iron_chestplate", "4393068e1f6bcc11",
            "iron_leggings", "4262b62a438d1088",
            "iron_boots", "0983cc85e5167381",
            "bone_helmet", "2f9d83e49d2b8dbb",
            "bone_chestplate", "a6d39dc53ace5bf3",
            "bone_leggings", "be2b47132fae568a",
            "bone_boots", "77b698ba4541e7fd"
        );

        assertEquals(expected.keySet(), ArmorPartModel.modelKeys(), "digest 表必须覆盖全部运行时模型");
        expected.forEach((modelKey, digest) -> assertEquals(
            digest,
            cubeDigest(modelKey),
            modelKey + " 任一 mount/坐标/尺寸/UV 漂移都必须撞红"
        ));
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

    private static String cubeDigest(String modelKey) {
        long hash = 0xcbf29ce484222325L;
        for (ArmorPartModel.ArmorCube cube : ArmorPartModel.cubes(modelKey)) {
            hash = fnv1a(hash, cube.mount().ordinal());
            hash = fnv1a(hash, Float.floatToIntBits(cube.ox()));
            hash = fnv1a(hash, Float.floatToIntBits(cube.oy()));
            hash = fnv1a(hash, Float.floatToIntBits(cube.oz()));
            hash = fnv1a(hash, Float.floatToIntBits(cube.sx()));
            hash = fnv1a(hash, Float.floatToIntBits(cube.sy()));
            hash = fnv1a(hash, Float.floatToIntBits(cube.sz()));
            hash = fnv1a(hash, cube.u());
            hash = fnv1a(hash, cube.v());
        }
        return String.format("%016x", hash);
    }

    private static long fnv1a(long hash, int value) {
        for (int byteIndex = 0; byteIndex < Integer.BYTES; byteIndex++) {
            hash ^= value & 0xffL;
            hash *= 0x100000001b3L;
            value >>>= Byte.SIZE;
        }
        return hash;
    }

    @Test
    void supportAndBuildRejectUnknownKeys() {
        assertFalse(ArmorPartModel.supports(null));
        assertFalse(ArmorPartModel.supports("missing"));
        assertThrows(IllegalArgumentException.class, () -> ArmorPartModel.buildModelPart("missing"));
    }

}
