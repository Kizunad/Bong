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
    void toolModelRegistryAxeBoneRegistered() {
        assertTrue(BongWeaponModelRegistry.get("axe_bone").isPresent(),
            "axe_bone should be registered in BongWeaponModelRegistry");
        BongWeaponModelRegistry.Entry entry = BongWeaponModelRegistry.get("axe_bone").orElseThrow();
        assertEquals("item/wooden_axe", entry.vanillaModelPath(),
            "axe_bone vanilla model path should be item/wooden_axe");
        assertEquals("bong:models/item/axe_bone/axe_bone.obj", entry.bongObjModelPath(),
            "axe_bone bong OBJ path mismatch");
    }

    @Test
    void toolModelRegistryPickaxeIronRegistered() {
        assertTrue(BongWeaponModelRegistry.get("pickaxe_iron").isPresent(),
            "pickaxe_iron should be registered in BongWeaponModelRegistry");
        BongWeaponModelRegistry.Entry entry = BongWeaponModelRegistry.get("pickaxe_iron").orElseThrow();
        assertEquals("item/iron_pickaxe", entry.vanillaModelPath(),
            "pickaxe_iron vanilla model path should be item/iron_pickaxe");
        assertEquals("bong:models/item/pickaxe_iron/pickaxe_iron.obj", entry.bongObjModelPath(),
            "pickaxe_iron bong OBJ path mismatch");
    }

    @Test
    void axeBoneObjResourceExists() {
        var url = getClass().getClassLoader().getResource(
            "assets/bong/models/item/axe_bone/axe_bone.obj");
        assertTrue(url != null, "axe_bone.obj should exist in resources");
    }

    @Test
    void pickaxeIronObjResourceExists() {
        var url = getClass().getClassLoader().getResource(
            "assets/bong/models/item/pickaxe_iron/pickaxe_iron.obj");
        assertTrue(url != null, "pickaxe_iron.obj should exist in resources");
    }

    @Test
    void toolResourcePathsExistAndHostJsonPointsAtRegistryObj() throws IOException {
        for (String templateId : BongWeaponModelRegistry.TOOL_TEMPLATE_IDS) {
            assertTrue(BongWeaponModelRegistry.get(templateId).isPresent(),
                "tool " + templateId + " should be registered");
            BongWeaponModelRegistry.Entry entry = BongWeaponModelRegistry.get(templateId).orElseThrow();
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
    void toolTemplateIdsSetMatchesRegisteredTools() {
        Set<String> expected = Set.of("axe_bone", "pickaxe_bone", "axe_iron", "pickaxe_iron");
        assertEquals(expected, BongWeaponModelRegistry.TOOL_TEMPLATE_IDS,
            "expected TOOL_TEMPLATE_IDS=" + expected + ", actual=" + BongWeaponModelRegistry.TOOL_TEMPLATE_IDS);
        for (String templateId : BongWeaponModelRegistry.TOOL_TEMPLATE_IDS) {
            assertTrue(BongWeaponModelRegistry.get(templateId).isPresent(),
                "expected " + templateId + " in registry because it's in TOOL_TEMPLATE_IDS, actual isPresent=false");
        }
    }

    // ─── #3 手持盾无盾模型：盾牌也走本注册表 fake-stack + SML 劫持渲染链 ───
    //
    // 锁住：盾牌模板齐备、宿主 item model 路径正确、OBJ/MTL/贴图资源到位、且宿主路径进入
    // SML vanillaModelPaths()——后者是「SML 真去劫持该 vanilla item model」的唯一开关，缺它
    // fake stack 会渲染成原版宿主 item（nautilus_shell/phantom_membrane）而非盾牌（接线孤岛）。
    // hostItem()（→ Items.X）触原版注册表需 MC Bootstrap，client 测试环境没有（见
    // BlockVanillaIconMapTest），故真 ItemStack 合成由 e2e/真游戏覆盖；此处只锁注册表数据 + 资源。

    @Test
    void shieldTemplateIdsSetMatchesRegisteredShields() {
        Set<String> expected = Set.of("wooden_shield", "bone_shield");
        assertEquals(expected, BongWeaponModelRegistry.SHIELD_TEMPLATE_IDS,
            "expected SHIELD_TEMPLATE_IDS=" + expected + ", actual=" + BongWeaponModelRegistry.SHIELD_TEMPLATE_IDS);
        for (String templateId : BongWeaponModelRegistry.SHIELD_TEMPLATE_IDS) {
            assertTrue(BongWeaponModelRegistry.get(templateId).isPresent(),
                "expected " + templateId + " in registry because it's in SHIELD_TEMPLATE_IDS, actual isPresent=false");
        }
    }

    @Test
    void shieldRegistryMapsToExpectedHostsAndObjPaths() {
        BongWeaponModelRegistry.Entry wooden = BongWeaponModelRegistry.get("wooden_shield").orElseThrow();
        assertEquals("item/nautilus_shell", wooden.vanillaModelPath(), "木盾宿主 model 路径");
        assertEquals("bong:models/item/wooden_shield/wooden_shield.obj", wooden.bongObjModelPath(), "木盾 OBJ 路径");

        BongWeaponModelRegistry.Entry bone = BongWeaponModelRegistry.get("bone_shield").orElseThrow();
        assertEquals("item/phantom_membrane", bone.vanillaModelPath(), "骨盾宿主 model 路径");
        assertEquals("bong:models/item/bone_shield/bone_shield.obj", bone.bongObjModelPath(), "骨盾 OBJ 路径");
    }

    @Test
    void shieldResourcePathsExistAndHostJsonPointsAtRegistryObj() throws IOException {
        for (String templateId : BongWeaponModelRegistry.SHIELD_TEMPLATE_IDS) {
            BongWeaponModelRegistry.Entry entry = BongWeaponModelRegistry.get(templateId).orElseThrow();
            assertTrue(entry.bongObjModelPath() != null, templateId + " 盾必须有 OBJ（否则不进 SML scope）");
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
    void shieldHostsAreInSmlVanillaModelScope() {
        // 关键接线断言：SML 仅劫持 LOAD_SCOPE 声明过的 vanilla model（见 WeaponRenderBootstrap）。
        // 盾的宿主路径必须出现在 vanillaModelPaths()，否则盾 fake stack 渲染成原版海螺壳/幻翼膜。
        Set<String> paths = BongWeaponModelRegistry.vanillaModelPaths();
        assertTrue(paths.contains("item/nautilus_shell"),
            "木盾宿主 item/nautilus_shell 必须在 SML vanillaModelPaths 内，实际=" + paths);
        assertTrue(paths.contains("item/phantom_membrane"),
            "骨盾宿主 item/phantom_membrane 必须在 SML vanillaModelPaths 内，实际=" + paths);
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
