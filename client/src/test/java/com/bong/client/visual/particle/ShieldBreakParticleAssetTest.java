package com.bong.client.visual.particle;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-shield-block-v1 §P3：破盾粒子资产存在性测试。
 *
 * <p>确保 wood_debris.png 和 wood_debris.json particle descriptor 均存在且引用一致。
 * 在 headless 测试中无法真正加载 MC atlas，但可确保文件不缺失不断链。
 *
 * <p>注意：audio_recipes/shield_break.json 已删除（死资产）——
 * ShieldBrokenHandler 走内联 AudioRecipe（shield_break_wood / shield_break_bone），
 * 从不读取该 JSON 文件。相关断言也一并删除，防止误导。
 */
class ShieldBreakParticleAssetTest {

    private static final Path PARTICLE_ROOT   = Path.of("src/main/resources/assets/bong/particles");
    private static final Path TEXTURE_ROOT    = Path.of("src/main/resources/assets/bong/textures/particle");
    private static final Path ATLAS_FILE      = Path.of(
        "src/main/resources/assets/minecraft/atlases/particles.json");

    // ---- wood_debris 粒子贴图 + 描述符 --------------------------------

    @Test
    void woodDebrisTexture_exists() {
        Path png = TEXTURE_ROOT.resolve("wood_debris.png");
        assertTrue(Files.isRegularFile(png),
            "破盾粒子贴图 bong:particle/wood_debris 不存在，缺少文件: " + png
                + "。该贴图必须 commit（64x64 紧凑木屑簇，RGB 纯白可染色）");
    }

    @Test
    void woodDebrisParticleDescriptor_exists() {
        Path json = PARTICLE_ROOT.resolve("wood_debris.json");
        assertTrue(Files.isRegularFile(json),
            "粒子 descriptor bong/particles/wood_debris.json 不存在: " + json);
    }

    @Test
    void woodDebrisParticleDescriptor_referencesCorrectTexture() throws IOException {
        Path json = PARTICLE_ROOT.resolve("wood_debris.json");
        String content = Files.readString(json, StandardCharsets.UTF_8);
        JsonObject root = JsonParser.parseString(content).getAsJsonObject();
        assertTrue(root.has("textures"),
            "wood_debris.json 应有 textures 数组");
        String textureRef = root.get("textures").getAsJsonArray().get(0).getAsString();
        assertEquals("bong:particle/wood_debris", textureRef,
            "wood_debris.json 贴图引用应为 bong:particle/wood_debris，实际: " + textureRef);
    }

    // ---- atlas 注册 -------------------------------------------------------

    @Test
    void woodDebrisTexture_registeredInAtlas() throws IOException {
        String atlasContent = Files.readString(ATLAS_FILE, StandardCharsets.UTF_8);
        assertTrue(atlasContent.contains("bong:particle/wood_debris"),
            "bong:particle/wood_debris 应在 minecraft/atlases/particles.json 中注册，"
                + "否则 MC 粒子 atlas 不会加载该贴图，ShieldWoodShatterPlayer 会静默渲染空白粒子。"
                + "确认 particles.json 中有 {\"type\":\"single\",\"resource\":\"bong:particle/wood_debris\"}");
    }

    // ---- ShieldWoodShatterPlayer EVENT_ID 命名规范 ----------------------

    @Test
    void shieldWoodShatterPlayer_eventId_hasBongNamespace() {
        assertEquals("bong", ShieldWoodShatterPlayer.EVENT_ID.getNamespace(),
            "ShieldWoodShatterPlayer.EVENT_ID namespace 应为 bong");
        assertEquals("shield_break_wood", ShieldWoodShatterPlayer.EVENT_ID.getPath(),
            "ShieldWoodShatterPlayer.EVENT_ID path 应为 shield_break_wood");
    }
}
