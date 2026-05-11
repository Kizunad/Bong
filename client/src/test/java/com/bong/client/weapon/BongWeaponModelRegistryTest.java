package com.bong.client.weapon;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BongWeaponModelRegistryTest {
    private static final Path RESOURCES = Path.of("src", "main", "resources");
    private static final Map<String, ExpectedWeaponResource> V1_EXPECTED_RESOURCES = expectedResources();

    @Test
    void registryCoversExactlyTheNineV1WeaponTemplates() {
        assertEquals(V1_EXPECTED_RESOURCES.keySet(), BongWeaponModelRegistry.V1_WEAPON_TEMPLATE_IDS);

        for (Map.Entry<String, ExpectedWeaponResource> expected : V1_EXPECTED_RESOURCES.entrySet()) {
            BongWeaponModelRegistry.Entry actual = BongWeaponModelRegistry.get(expected.getKey()).orElseThrow();

            assertEquals(expected.getKey(), actual.templateId());
            assertEquals(expected.getValue().vanillaModelPath(), actual.vanillaModelPath());
            assertEquals(expected.getValue().bongObjModelPath(), actual.bongObjModelPath());
            if (actual.bongObjModelPath() != null) {
                assertFalse(actual.bongObjModelPath().contains("placeholder"));
                assertFalse(actual.bongObjModelPath().contains("wooden_totem"));
            }
        }
    }

    @Test
    void legacyRustedBladeIsRegisteredButExcludedFromV1WeaponSet() {
        BongWeaponModelRegistry.Entry rustedBlade = BongWeaponModelRegistry.get("rusted_blade").orElseThrow();

        assertFalse(BongWeaponModelRegistry.V1_WEAPON_TEMPLATE_IDS.contains("rusted_blade"));
        assertEquals("item/netherite_sword", rustedBlade.vanillaModelPath());
        assertEquals("bong:models/item/rusted_blade/rusted_blade.obj", rustedBlade.bongObjModelPath());
    }

    @Test
    void v1WeaponResourcePathsExistAndHostJsonPointsAtRegistryObj() throws IOException {
        for (String templateId : BongWeaponModelRegistry.V1_WEAPON_TEMPLATE_IDS) {
            BongWeaponModelRegistry.Entry entry = BongWeaponModelRegistry.get(templateId).orElseThrow();
            if (entry.bongObjModelPath() == null) {
                continue;
            }
            JsonObject hostJson = readHostJson(entry);

            assertEquals("sml:builtin/obj", hostJson.get("parent").getAsString(), templateId + " host parent");
            assertEquals(entry.bongObjModelPath(), hostJson.get("model").getAsString(), templateId + " host model");
            assertTrue(Files.isRegularFile(bongResourcePath(entry.bongObjModelPath())), templateId + " OBJ missing");
            assertTrue(Files.isRegularFile(bongResourcePath(mtlPath(entry.bongObjModelPath()))), templateId + " MTL missing");
            assertTrue(Files.isDirectory(textureDir(templateId)), templateId + " texture dir missing");
            assertTrue(hasPngTexture(textureDir(templateId)), templateId + " texture dir has no PNG");
        }
    }

    @Test
    void vanillaModelPathSetIncludesOnlyObjBackedHosts() {
        Set<String> paths = BongWeaponModelRegistry.vanillaModelPaths();

        for (String templateId : BongWeaponModelRegistry.V1_WEAPON_TEMPLATE_IDS) {
            BongWeaponModelRegistry.Entry entry = BongWeaponModelRegistry.get(templateId).orElseThrow();
            assertEquals(entry.bongObjModelPath() != null, paths.contains(entry.vanillaModelPath()));
        }
        assertTrue(paths.contains(BongWeaponModelRegistry.get("rusted_blade").orElseThrow().vanillaModelPath()));
    }

    private static Map<String, ExpectedWeaponResource> expectedResources() {
        Map<String, ExpectedWeaponResource> out = new LinkedHashMap<>();
        out.put("iron_sword", new ExpectedWeaponResource("item/iron_sword", "bong:models/item/iron_sword/iron_sword.obj"));
        out.put("bronze_saber", new ExpectedWeaponResource("item/golden_sword", "bong:models/item/bronze_saber/bronze_saber.obj"));
        out.put("wooden_staff", new ExpectedWeaponResource("item/totem_of_undying", "bong:models/item/wooden_staff/wooden_staff.obj"));
        out.put("bone_dagger", new ExpectedWeaponResource("item/bone", "bong:models/item/bone_dagger/bone_dagger.obj"));
        out.put("hand_wrap", new ExpectedWeaponResource("item/leather", "bong:models/item/hand_wrap/hand_wrap.obj"));
        out.put("bone_sword", new ExpectedWeaponResource("item/stone_sword", null));
        out.put("lingmu_sword", new ExpectedWeaponResource("item/wooden_sword", null));
        out.put("spirit_sword", new ExpectedWeaponResource("item/nether_star", "bong:models/item/spirit_sword/spirit_sword.obj"));
        out.put("flying_sword_feixuan", new ExpectedWeaponResource("item/diamond_sword", "bong:models/item/flying_sword_feixuan/flying_sword_feixuan.obj"));
        return Collections.unmodifiableMap(out);
    }

    private static JsonObject readHostJson(BongWeaponModelRegistry.Entry entry) throws IOException {
        Path path = RESOURCES.resolve("assets/minecraft/models").resolve(entry.vanillaModelPath() + ".json");

        assertTrue(Files.isRegularFile(path), entry.templateId() + " host JSON missing at " + path);
        return JsonParser.parseString(Files.readString(path)).getAsJsonObject();
    }

    private static Path bongResourcePath(String resourceId) {
        String prefix = "bong:";

        assertTrue(resourceId.startsWith(prefix), "resource id should be in bong namespace: " + resourceId);
        return RESOURCES.resolve("assets/bong").resolve(resourceId.substring(prefix.length()));
    }

    private static String mtlPath(String objPath) {
        assertTrue(objPath.endsWith(".obj"), "OBJ path should end with .obj: " + objPath);
        return objPath.substring(0, objPath.length() - ".obj".length()) + ".mtl";
    }

    private static Path textureDir(String templateId) {
        return RESOURCES.resolve("assets/bong/textures/item").resolve(templateId);
    }

    private static boolean hasPngTexture(Path dir) throws IOException {
        try (var files = Files.list(dir)) {
            return files.anyMatch(path -> path.getFileName().toString().endsWith(".png"));
        }
    }

    private record ExpectedWeaponResource(String vanillaModelPath, String bongObjModelPath) {
    }
}
