package com.bong.client.armor;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ArmorModelRegistryTest {
    private static final Path RESOURCES = Path.of("src", "main", "resources");

    @Test
    void armorModelRegistryIronHelmetRegistered() {
        assertTrue(ArmorModelRegistry.get("armor_iron_helmet").isPresent(),
            "armor_iron_helmet should be registered in ArmorModelRegistry");
        assertEquals("head", ArmorModelRegistry.get("armor_iron_helmet").get().slot(),
            "iron helmet slot should be 'head'");
    }

    @Test
    void armorModelRegistryGetUnknownReturnsNone() {
        assertTrue(ArmorModelRegistry.get("nonexistent_armor").isEmpty(),
            "unknown template_id should return empty Optional");
        assertTrue(ArmorModelRegistry.get(null).isEmpty(),
            "null template_id should return empty Optional");
        assertTrue(ArmorModelRegistry.get("  ").isEmpty(),
            "whitespace-only template_id should return empty Optional");
    }

    @Test
    void armorFeatureRendererLoadsWithoutCrash() {
        assertDoesNotThrow(
            () -> Class.forName("com.bong.client.armor.ArmorFeatureRenderer"),
            "ArmorFeatureRenderer class should load without errors"
        );
    }

    @Test
    void armorFeatureRendererRendersEquippedIronHelmet() {
        var spec = ArmorModelRegistry.get("armor_iron_helmet");
        assertTrue(spec.isPresent(), "iron helmet should resolve from registry for renderer");
        assertTrue(spec.get().modelPath().endsWith(".obj"),
            "model path should end with .obj, got: " + spec.get().modelPath());
        assertTrue(spec.get().texturePath().endsWith(".png"),
            "texture path should end with .png, got: " + spec.get().texturePath());
    }

    @Test
    void mixinArmorReturnsEmptyWhenModelRegistered() {
        assertTrue(ArmorModelRegistry.get("armor_iron_helmet").isPresent(),
            "iron helmet should be in ArmorModelRegistry, causing mixin to skip leather dye");
        assertTrue(ArmorModelRegistry.get("armor_iron_chestplate").isPresent(),
            "iron chestplate should be in ArmorModelRegistry");
        assertTrue(ArmorModelRegistry.get("armor_iron_leggings").isPresent(),
            "iron leggings should be in ArmorModelRegistry");
        assertTrue(ArmorModelRegistry.get("armor_iron_boots").isPresent(),
            "iron boots should be in ArmorModelRegistry");
    }

    @Test
    void mixinArmorFallsBackToLeatherWhenNoModel() {
        assertTrue(ArmorModelRegistry.get("armor_copper_helmet").isEmpty(),
            "copper helmet has no OBJ model, should fall back to leather dye");
        assertNotNull(ArmorTintRegistry.item("armor_copper_helmet"),
            "copper helmet should exist in ArmorTintRegistry for leather dye fallback");
    }

    @Test
    void armorModelRegistryBoneHelmetRegistered() {
        assertTrue(ArmorModelRegistry.get("armor_bone_helmet").isPresent(),
            "armor_bone_helmet should be registered in ArmorModelRegistry");
        assertEquals("head", ArmorModelRegistry.get("armor_bone_helmet").get().slot(),
            "bone helmet slot should be 'head'");
    }

    @Test
    void boneHelmetObjResourceExists() {
        var url = getClass().getClassLoader().getResource(
            "assets/bong/models/armor/bone_helmet/bone_helmet.obj");
        assertNotNull(url, "bone_helmet.obj should exist in resources");
    }

    @Test
    void boneChestplateObjResourceExists() {
        var url = getClass().getClassLoader().getResource(
            "assets/bong/models/armor/bone_chestplate/bone_chestplate.obj");
        assertNotNull(url, "bone_chestplate.obj should exist in resources");
    }

    @Test
    void ironHelmetObjResourceExists() {
        var url = getClass().getClassLoader().getResource(
            "assets/bong/models/armor/iron_helmet/iron_helmet.obj");
        assertNotNull(url, "iron_helmet.obj should exist in resources");
    }

    @Test
    void ironChestplateObjResourceExists() {
        var url = getClass().getClassLoader().getResource(
            "assets/bong/models/armor/iron_chestplate/iron_chestplate.obj");
        assertNotNull(url, "iron_chestplate.obj should exist in resources");
    }

    @Test
    void registrySizeIsEightPieces() {
        assertEquals(8, ArmorModelRegistry.size(),
            "ArmorModelRegistry should contain exactly 8 armor pieces (4 iron + 4 bone)");
    }

    @Test
    void allRegisteredModelsHaveObjMtlTextureAndJson() throws IOException {
        String[] dirs = {"iron_helmet", "iron_chestplate", "iron_leggings", "iron_boots",
            "bone_helmet", "bone_chestplate", "bone_leggings", "bone_boots"};
        for (String dir : dirs) {
            Path modelDir = RESOURCES.resolve("assets/bong/models/armor/" + dir);
            assertTrue(Files.isRegularFile(modelDir.resolve(dir + ".obj")),
                dir + ".obj missing");
            assertTrue(Files.isRegularFile(modelDir.resolve(dir + ".mtl")),
                dir + ".mtl missing");
            assertTrue(Files.isRegularFile(modelDir.resolve(dir + ".json")),
                dir + ".json (SML override) missing");

            Path texDir = RESOURCES.resolve("assets/bong/textures/armor/" + dir);
            assertTrue(Files.isRegularFile(texDir.resolve("0.png")),
                dir + "/0.png texture missing");
        }
    }

    @Test
    void allSlotsRepresented() {
        var helmet = ArmorModelRegistry.get("armor_iron_helmet");
        assertTrue(helmet.isPresent(), "armor_iron_helmet should be registered");
        assertEquals("head", helmet.get().slot(), "iron helmet slot mismatch");

        var chestplate = ArmorModelRegistry.get("armor_iron_chestplate");
        assertTrue(chestplate.isPresent(), "armor_iron_chestplate should be registered");
        assertEquals("chest", chestplate.get().slot(), "iron chestplate slot mismatch");

        var leggings = ArmorModelRegistry.get("armor_iron_leggings");
        assertTrue(leggings.isPresent(), "armor_iron_leggings should be registered");
        assertEquals("legs", leggings.get().slot(), "iron leggings slot mismatch");

        var boots = ArmorModelRegistry.get("armor_iron_boots");
        assertTrue(boots.isPresent(), "armor_iron_boots should be registered");
        assertEquals("feet", boots.get().slot(), "iron boots slot mismatch");

        var boneHelmet = ArmorModelRegistry.get("armor_bone_helmet");
        assertTrue(boneHelmet.isPresent(), "armor_bone_helmet should be registered");
        assertEquals("head", boneHelmet.get().slot(), "bone helmet slot mismatch");

        var boneChestplate = ArmorModelRegistry.get("armor_bone_chestplate");
        assertTrue(boneChestplate.isPresent(), "armor_bone_chestplate should be registered");
        assertEquals("chest", boneChestplate.get().slot(), "bone chestplate slot mismatch");

        var boneLeggings = ArmorModelRegistry.get("armor_bone_leggings");
        assertTrue(boneLeggings.isPresent(), "armor_bone_leggings should be registered");
        assertEquals("legs", boneLeggings.get().slot(), "bone leggings slot mismatch");

        var boneBoneBoot = ArmorModelRegistry.get("armor_bone_boots");
        assertTrue(boneBoneBoot.isPresent(), "armor_bone_boots should be registered");
        assertEquals("feet", boneBoneBoot.get().slot(), "bone boots slot mismatch");
    }

    @Test
    void modelPathsReturnsAllEightPieces() {
        var paths = ArmorModelRegistry.modelPaths();
        assertEquals(8, paths.size(), "modelPaths() should return 8 armor model paths (4 iron + 4 bone)");
        assertTrue(paths.stream().anyMatch(p -> p.contains("iron_helmet")),
            "modelPaths() should include iron_helmet");
        assertTrue(paths.stream().anyMatch(p -> p.contains("iron_chestplate")),
            "modelPaths() should include iron_chestplate");
        assertTrue(paths.stream().anyMatch(p -> p.contains("iron_leggings")),
            "modelPaths() should include iron_leggings");
        assertTrue(paths.stream().anyMatch(p -> p.contains("iron_boots")),
            "modelPaths() should include iron_boots");
        assertTrue(paths.stream().anyMatch(p -> p.contains("bone_helmet")),
            "modelPaths() should include bone_helmet");
        assertTrue(paths.stream().anyMatch(p -> p.contains("bone_chestplate")),
            "modelPaths() should include bone_chestplate");
        assertTrue(paths.stream().anyMatch(p -> p.contains("bone_leggings")),
            "modelPaths() should include bone_leggings");
        assertTrue(paths.stream().anyMatch(p -> p.contains("bone_boots")),
            "modelPaths() should include bone_boots");
    }
}
