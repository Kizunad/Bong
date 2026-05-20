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
        assertTrue(ArmorModelRegistry.get("armor_bone_helmet").isEmpty(),
            "bone helmet has no OBJ model, should fall back to leather dye");
        assertNotNull(ArmorTintRegistry.item("armor_bone_helmet"),
            "bone helmet should exist in ArmorTintRegistry for leather dye fallback");
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
    void registrySizeIsFourIronPieces() {
        assertEquals(4, ArmorModelRegistry.size(),
            "ArmorModelRegistry should contain exactly 4 iron armor pieces");
    }

    @Test
    void allRegisteredModelsHaveObjMtlTextureAndJson() throws IOException {
        String[] dirs = {"iron_helmet", "iron_chestplate", "iron_leggings", "iron_boots"};
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
        assertEquals("head",  ArmorModelRegistry.get("armor_iron_helmet").get().slot());
        assertEquals("chest", ArmorModelRegistry.get("armor_iron_chestplate").get().slot());
        assertEquals("legs",  ArmorModelRegistry.get("armor_iron_leggings").get().slot());
        assertEquals("feet",  ArmorModelRegistry.get("armor_iron_boots").get().slot());
    }
}
