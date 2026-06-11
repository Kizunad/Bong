package com.bong.client.network;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-shield-block-v1 §P4：格挡命中 audio_recipe JSON 资产存在性 + 结构测试。
 *
 * <p>确保 shield_block.json（木盾）和 shield_block_bone.json（骨盾）存在、
 * pitch/layer 差异化配置正确、bus/category 合法。
 */
class ShieldBlockAudioRecipeAssetTest {

    private static final Path AUDIO_RECIPES =
        Path.of("src/main/resources/assets/bong/audio_recipes");

    // ── 文件存在性 ────────────────────────────────────────────────────────

    @Test
    void shieldBlockJson_exists() {
        Path file = AUDIO_RECIPES.resolve("shield_block.json");
        assertTrue(Files.isRegularFile(file),
            "木盾格挡命中音效 assets/bong/audio_recipes/shield_block.json 不存在；"
                + "该文件必须 commit（木盾 pitch 0.9，与骨盾 1.3 形成差异化）");
    }

    @Test
    void shieldBlockBoneJson_exists() {
        Path file = AUDIO_RECIPES.resolve("shield_block_bone.json");
        assertTrue(Files.isRegularFile(file),
            "骨盾格挡命中音效 assets/bong/audio_recipes/shield_block_bone.json 不存在；"
                + "该文件必须 commit（骨盾 pitch 1.3 + skeleton.hurt 第二层）");
    }

    // ── 木盾规格 ──────────────────────────────────────────────────────────

    @Test
    void shieldBlock_id_matches_filename() throws IOException {
        JsonObject root = readJson("shield_block.json");
        assertEquals("shield_block", root.get("id").getAsString(),
            "shield_block.json id 字段应为 shield_block");
    }

    @Test
    void shieldBlock_hasExactlyOneLayer() throws IOException {
        JsonObject root = readJson("shield_block.json");
        assertEquals(1, root.get("layers").getAsJsonArray().size(),
            "木盾 shield_block.json 应有 1 layer（单一 item.shield.block 音），实际 layers="
                + root.get("layers").getAsJsonArray().size());
    }

    @Test
    void shieldBlock_layer1_soundIsShieldBlock() throws IOException {
        JsonObject root = readJson("shield_block.json");
        String sound = root.get("layers").getAsJsonArray()
            .get(0).getAsJsonObject().get("sound").getAsString();
        assertEquals("minecraft:item.shield.block", sound,
            "木盾 layer1 sound 应为 minecraft:item.shield.block，实际=" + sound);
    }

    @Test
    void shieldBlock_layer1_pitch_is0point9() throws IOException {
        JsonObject root = readJson("shield_block.json");
        float pitch = root.get("layers").getAsJsonArray()
            .get(0).getAsJsonObject().get("pitch").getAsFloat();
        assertEquals(0.9f, pitch, 0.001f,
            "木盾 layer1 pitch 应 0.9（与骨盾 1.3 形成可感知差异），实际=" + pitch);
    }

    @Test
    void shieldBlock_busIsCombat() throws IOException {
        JsonObject root = readJson("shield_block.json");
        assertEquals("COMBAT", root.get("bus").getAsString(),
            "木盾 shield_block.json bus 应为 COMBAT，实际=" + root.get("bus").getAsString());
    }

    // ── 骨盾规格 ──────────────────────────────────────────────────────────

    @Test
    void shieldBlockBone_id_matches_filename() throws IOException {
        JsonObject root = readJson("shield_block_bone.json");
        assertEquals("shield_block_bone", root.get("id").getAsString(),
            "shield_block_bone.json id 字段应为 shield_block_bone");
    }

    @Test
    void shieldBlockBone_hasTwoLayers() throws IOException {
        JsonObject root = readJson("shield_block_bone.json");
        assertEquals(2, root.get("layers").getAsJsonArray().size(),
            "骨盾 shield_block_bone.json 应有 2 layers（layer1 + skeleton.hurt 第二层），实际 layers="
                + root.get("layers").getAsJsonArray().size());
    }

    @Test
    void shieldBlockBone_layer1_soundIsShieldBlock() throws IOException {
        JsonObject root = readJson("shield_block_bone.json");
        String sound = root.get("layers").getAsJsonArray()
            .get(0).getAsJsonObject().get("sound").getAsString();
        assertEquals("minecraft:item.shield.block", sound,
            "骨盾 layer1 sound 应为 minecraft:item.shield.block，实际=" + sound);
    }

    @Test
    void shieldBlockBone_layer1_pitch_is1point3() throws IOException {
        JsonObject root = readJson("shield_block_bone.json");
        float pitch = root.get("layers").getAsJsonArray()
            .get(0).getAsJsonObject().get("pitch").getAsFloat();
        assertEquals(1.3f, pitch, 0.001f,
            "骨盾 layer1 pitch 应 1.3（脆响，高于木盾 0.9），实际=" + pitch);
    }

    @Test
    void shieldBlockBone_layer2_soundIsSkeletonHurt() throws IOException {
        JsonObject root = readJson("shield_block_bone.json");
        String sound = root.get("layers").getAsJsonArray()
            .get(1).getAsJsonObject().get("sound").getAsString();
        assertEquals("minecraft:entity.skeleton.hurt", sound,
            "骨盾 layer2 sound 应为 minecraft:entity.skeleton.hurt（骨感回响），实际=" + sound);
    }

    @Test
    void shieldBlockBone_layer2_volume_is0point3() throws IOException {
        JsonObject root = readJson("shield_block_bone.json");
        float volume = root.get("layers").getAsJsonArray()
            .get(1).getAsJsonObject().get("volume").getAsFloat();
        assertEquals(0.3f, volume, 0.001f,
            "骨盾 layer2 volume 应 0.3（骨感回响轻于主层），实际=" + volume);
    }

    @Test
    void shieldBlockBone_layer2_delayTicks_is1() throws IOException {
        JsonObject root = readJson("shield_block_bone.json");
        int delay = root.get("layers").getAsJsonArray()
            .get(1).getAsJsonObject().get("delay_ticks").getAsInt();
        assertEquals(1, delay,
            "骨盾 layer2 delay_ticks 应为 1，实际=" + delay);
    }

    @Test
    void shieldBlockBone_busIsCombat() throws IOException {
        JsonObject root = readJson("shield_block_bone.json");
        assertEquals("COMBAT", root.get("bus").getAsString(),
            "骨盾 shield_block_bone.json bus 应为 COMBAT，实际=" + root.get("bus").getAsString());
    }

    // ── 差异化关键断言 ───────────────────────────────────────────────────────

    @Test
    void woodVsBone_layer1Pitch_areDifferentAndDistinguishable() throws IOException {
        float woodPitch = readJson("shield_block.json")
            .get("layers").getAsJsonArray()
            .get(0).getAsJsonObject().get("pitch").getAsFloat();
        float bonePitch = readJson("shield_block_bone.json")
            .get("layers").getAsJsonArray()
            .get(0).getAsJsonObject().get("pitch").getAsFloat();

        assertNotEquals(woodPitch, bonePitch, 0.001f,
            "木盾 pitch=" + woodPitch + " 与骨盾 pitch=" + bonePitch + " 应不同（材质差异化可感知）");
        assertTrue(bonePitch > woodPitch,
            "骨盾 pitch(" + bonePitch + ") 应 > 木盾 pitch(" + woodPitch
                + ")（骨盾脆响更高，玩家可感知两者差异）");
    }

    @Test
    void woodVsBone_layerCountDiffers() throws IOException {
        int woodLayers = readJson("shield_block.json").get("layers").getAsJsonArray().size();
        int boneLayers = readJson("shield_block_bone.json").get("layers").getAsJsonArray().size();
        assertNotEquals(woodLayers, boneLayers,
            "木盾层数(" + woodLayers + ") 与骨盾层数(" + boneLayers + ") 应不同（骨盾有额外骨感回响层）");
    }

    // ── helpers ────────────────────────────────────────────────────────────

    private static JsonObject readJson(String filename) throws IOException {
        Path file = AUDIO_RECIPES.resolve(filename);
        String content = Files.readString(file, StandardCharsets.UTF_8);
        return JsonParser.parseString(content).getAsJsonObject();
    }
}
