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
    void objPlaceholderResourcesAreGone() throws Exception {
        Path armorModels = RESOURCES.resolve("assets/bong/models/armor");
        if (!Files.exists(armorModels)) {
            return;
        }
        try (var paths = Files.walk(armorModels)) {
            assertFalse(paths.anyMatch(path -> {
                String name = path.getFileName().toString();
                return name.endsWith(".obj") || name.endsWith(".mtl");
            }), "ModelPart 路线不应残留 OBJ/MTL 方盒占位");
        }
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
}
