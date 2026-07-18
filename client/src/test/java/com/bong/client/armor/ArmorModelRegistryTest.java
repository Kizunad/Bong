package com.bong.client.armor;

import com.bong.client.inventory.model.EquipSlotType;
import net.minecraft.entity.EquipmentSlot;
import org.junit.jupiter.api.Test;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Set;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ArmorModelRegistryTest {
    private static final Path RESOURCES = Path.of("src", "main", "resources");

    @Test
    void registryContainsExactlyTwoMaterialsAcrossAllFourSlots() {
        assertEquals(8, ArmorModelRegistry.size(), "2 材质 × 4 槽必须恰好注册 8 件");
        for (String material : new String[]{"iron", "bone"}) {
            assertSpec(material, "helmet", EquipSlotType.HEAD);
            assertSpec(material, "chestplate", EquipSlotType.CHEST);
            assertSpec(material, "leggings", EquipSlotType.LEGS);
            assertSpec(material, "boots", EquipSlotType.FEET);
        }
    }

    @Test
    void lookupTrimsKnownIdsAndRejectsMissingInputs() {
        assertEquals("iron_helmet", ArmorModelRegistry.get("  armor_iron_helmet  ").orElseThrow().modelKey());
        assertTrue(ArmorModelRegistry.get("nonexistent_armor").isEmpty());
        assertTrue(ArmorModelRegistry.get(null).isEmpty());
        assertTrue(ArmorModelRegistry.get("  ").isEmpty());
    }

    @Test
    void everyRegistryModelKeyHasCubeTableAndTexture() {
        Set<String> registryKeys = ArmorModelRegistry.all().stream()
            .map(ArmorModelRegistry.ArmorModelSpec::modelKey)
            .collect(Collectors.toSet());
        assertEquals(ArmorPartModel.modelKeys(), registryKeys,
            "Registry modelKey 与 ArmorPartModel cube 表必须双向全覆盖");

        for (ArmorModelRegistry.ArmorModelSpec spec : ArmorModelRegistry.all()) {
            assertTrue(ArmorPartModel.supports(spec.modelKey()), spec.modelKey() + " 应能烘焙");
            assertDoesNotThrow(() -> ArmorPartModel.buildModelPart(spec.modelKey()));
            Path texture = RESOURCES.resolve("assets")
                .resolve(spec.texturePath().replace(':', '/'));
            assertTrue(Files.isRegularFile(texture), spec.texturePath() + " 贴图缺失");
        }
    }

    @Test
    void objPlaceholderResourcesAndArmorSmlScopeAreGone() throws Exception {
        Path armorModels = RESOURCES.resolve("assets/bong/models/armor");
        if (Files.exists(armorModels)) {
            try (var paths = Files.walk(armorModels)) {
                assertFalse(paths.anyMatch(path -> {
                    String name = path.getFileName().toString();
                    return name.endsWith(".obj") || name.endsWith(".mtl");
                }), "ModelPart 路线不应残留 OBJ/MTL 方盒占位");
            }
        }

        Path bootstrap = Path.of("src/main/java/com/bong/client/armor/ArmorRenderBootstrap.java");
        String bootstrapSource = Files.readString(bootstrap);
        assertFalse(bootstrapSource.contains("SpecialModelLoaderEvents"), "护甲 bootstrap 不得再注册 SML scope");
        assertFalse(bootstrapSource.contains("models/armor/"), "已删除的 OBJ 目录不得残留 scope 字符串");
    }

    @Test
    void unregisteredMaterialsKeepLeatherFallback() {
        assertTrue(ArmorModelRegistry.get("armor_copper_helmet").isEmpty());
        assertNotNull(ArmorTintRegistry.item("armor_copper_helmet"));
    }

    @Test
    void registeredMaterialsStillHaveSlotMatchedEmergencyFallbackData() {
        for (ArmorModelRegistry.ArmorModelSpec modelSpec : ArmorModelRegistry.all()) {
            ArmorTintRegistry.ArmorItemSpec tint = ArmorTintRegistry.item(modelSpec.templateId());
            assertNotNull(tint, modelSpec.templateId() + " 应保留 leather fallback 数据");
            assertEquals(vanillaSlot(modelSpec.slot()), tint.slot(), modelSpec.templateId() + " fallback 槽错位");
        }
    }

    @Test
    void ironBoneAndDyedLeatherFallbackUseThreeDistinctVisualRoutes() throws Exception {
        for (String piece : new String[]{"helmet", "chestplate", "leggings", "boots"}) {
            ArmorModelRegistry.ArmorModelSpec iron =
                ArmorModelRegistry.get("armor_iron_" + piece).orElseThrow();
            ArmorModelRegistry.ArmorModelSpec bone =
                ArmorModelRegistry.get("armor_bone_" + piece).orElseThrow();

            assertNotEquals(ArmorPartModel.cubes(iron.modelKey()), ArmorPartModel.cubes(bone.modelKey()),
                piece + " 的铁/骨 cube 轮廓必须不同");
            assertTrue(Files.mismatch(texturePath(iron), texturePath(bone)) >= 0,
                piece + " 的铁/骨贴图不得字节相同");

            String copperId = "armor_copper_" + piece;
            assertTrue(ArmorModelRegistry.get(copperId).isEmpty(), copperId + " 应继续走染色皮甲兜底");
            assertNotNull(ArmorTintRegistry.item(copperId), copperId + " 缺 leather fallback 规格");
        }

        assertEquals(3, Set.of(
            ArmorTintRegistry.tintForItemId("armor_iron_chestplate"),
            ArmorTintRegistry.tintForItemId("armor_bone_chestplate"),
            ArmorTintRegistry.tintForItemId("armor_copper_chestplate")
        ).size(), "铁、骨、染色皮甲兜底必须保留三种不同色相");
    }

    private static void assertSpec(String material, String piece, EquipSlotType expectedSlot) {
        String templateId = "armor_" + material + "_" + piece;
        ArmorModelRegistry.ArmorModelSpec spec = ArmorModelRegistry.get(templateId).orElseThrow();
        assertEquals(expectedSlot, spec.slot(), templateId + " 槽位错误");
        assertEquals(material + "_" + piece, spec.modelKey(), templateId + " modelKey 错误");
        assertEquals("bong:textures/armor/" + material + "_" + piece + "/0.png", spec.texturePath());
    }

    private static EquipmentSlot vanillaSlot(EquipSlotType slot) {
        return switch (slot) {
            case HEAD -> EquipmentSlot.HEAD;
            case CHEST -> EquipmentSlot.CHEST;
            case LEGS -> EquipmentSlot.LEGS;
            case FEET -> EquipmentSlot.FEET;
            default -> throw new IllegalArgumentException("not an armor slot: " + slot);
        };
    }

    private static Path texturePath(ArmorModelRegistry.ArmorModelSpec spec) {
        return RESOURCES.resolve("assets").resolve(spec.texturePath().replace(':', '/'));
    }
}
