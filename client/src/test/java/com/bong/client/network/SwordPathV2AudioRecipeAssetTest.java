package com.bong.client.network;

import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioRecipe;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SwordPathV2AudioRecipeAssetTest {
    private static final Path RECIPE_ROOT = Path.of("src/main/resources/assets/bong/audio_recipes");

    private static final List<String> RECIPE_IDS = List.of(
        "sword_bond_form",
        "sword_shatter",
        "sword_condense_edge",
        "sword_condense_hit",
        "sword_qi_slash",
        "sword_qi_slash_hit",
        "sword_resonance",
        "sword_manifest_summon",
        "sword_manifest_strike",
        "heaven_gate_charge_0s",
        "heaven_gate_charge_1s",
        "heaven_gate_charge_2s",
        "heaven_gate_flash",
        "heaven_gate_release",
        "heiwushi_melee_slash",
        "heiwushi_dark_barrage",
        "heiwushi_dark_vortex",
        "heiwushi_transform",
        "heiwushi_death",
        "sword_scroll_read"
    );

    @Test
    void swordPathV2AudioRecipesParseThroughWireEnvelope() throws IOException {
        // Guard: ensures this list stays in sync with actual RECIPE_IDS entries (copy-paste protection).
        assertEquals(20, RECIPE_IDS.size());
        for (String recipeId : RECIPE_IDS) {
            Path path = RECIPE_ROOT.resolve(recipeId + ".json");
            assertTrue(Files.isRegularFile(path), "missing sword-path-v2 audio recipe: " + path);

            JsonObject recipe = JsonParser.parseString(Files.readString(path)).getAsJsonObject();
            assertEquals(recipeId, recipe.get("id").getAsString(), recipeId + " id must match filename");

            JsonObject envelope = new JsonObject();
            envelope.addProperty("v", 1);
            envelope.addProperty("recipe_id", recipeId);
            envelope.addProperty("instance_id", 1);
            envelope.addProperty("volume_mul", 1.0);
            envelope.addProperty("pitch_shift", 0.0);
            envelope.add("recipe", recipe);

            AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(envelope.toString(), envelope.toString().length());
            assertTrue(parsed.isSuccess(), recipeId + " should parse as play_sound_recipe: " + parsed.errorMessage());
            AudioEventPayload.PlaySoundRecipe payload =
                assertInstanceOf(AudioEventPayload.PlaySoundRecipe.class, parsed.payload());
            AudioRecipe parsedRecipe = payload.recipe();
            assertEquals(recipeId, parsedRecipe.id());
            assertTrue(parsedRecipe.layers().size() >= 1 && parsedRecipe.layers().size() <= 8,
                recipeId + " must define 1..8 layers");
            assertEquals(AudioBus.COMBAT, parsedRecipe.bus(), recipeId + " must route through the combat bus");
            if (recipeId.startsWith("heiwushi_")) {
                assertEquals(AudioCategory.HOSTILE, parsedRecipe.category(), recipeId + " must use HOSTILE category");
            }
        }
    }

    // ── 边界测试：max-layer recipe（8 层）────────────────────────────────────

    @Test
    void audioRecipeAcceptsMaxLayers() {
        // 边界：layers.size() == 8（最大允许值）应成功解析。
        JsonObject recipe = new JsonObject();
        recipe.addProperty("id", "test_max_layers");
        JsonArray layers = new JsonArray();
        for (int i = 0; i < 8; i++) {
            layers.add(buildLayer("minecraft:block.note_block.harp", 0.8f, 1.0f, 0));
        }
        recipe.add("layers", layers);
        recipe.addProperty("priority", 50);
        recipe.addProperty("attenuation", "world_3d");
        recipe.addProperty("category", "PLAYERS");
        recipe.addProperty("bus", "COMBAT");

        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(
            buildEnvelope("test_max_layers", recipe).toString(),
            buildEnvelope("test_max_layers", recipe).toString().length());
        assertTrue(parsed.isSuccess(),
            "8-layer recipe (max boundary) should parse successfully, got: " + parsed.errorMessage());
        AudioEventPayload.PlaySoundRecipe payload =
            assertInstanceOf(AudioEventPayload.PlaySoundRecipe.class, parsed.payload());
        assertEquals(8, payload.recipe().layers().size(),
            "max-layer recipe must contain exactly 8 layers after parse");
    }

    // ── 错误分支：无效 layer 数量 ─────────────────────────────────────────────

    @Test
    void audioRecipeRejectsZeroLayers() {
        // 错误分支：layers.size() == 0 应被拒绝（parser 要求至少 1 层）。
        JsonObject recipe = new JsonObject();
        recipe.addProperty("id", "test_zero_layers");
        recipe.add("layers", new JsonArray());
        recipe.addProperty("priority", 50);
        recipe.addProperty("attenuation", "world_3d");
        recipe.addProperty("category", "PLAYERS");
        recipe.addProperty("bus", "COMBAT");

        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(
            buildEnvelope("test_zero_layers", recipe).toString(),
            buildEnvelope("test_zero_layers", recipe).toString().length());
        assertFalse(parsed.isSuccess(),
            "0-layer recipe should be rejected by parser, but got success");
    }

    @Test
    void audioRecipeRejectsNineLayersExceedingMax() {
        // 错误分支：layers.size() == 9（超出 max=8）应被拒绝。
        JsonObject recipe = new JsonObject();
        recipe.addProperty("id", "test_nine_layers");
        JsonArray layers = new JsonArray();
        for (int i = 0; i < 9; i++) {
            layers.add(buildLayer("minecraft:block.note_block.harp", 0.5f, 1.0f, 0));
        }
        recipe.add("layers", layers);
        recipe.addProperty("priority", 50);
        recipe.addProperty("attenuation", "world_3d");
        recipe.addProperty("category", "PLAYERS");
        recipe.addProperty("bus", "COMBAT");

        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(
            buildEnvelope("test_nine_layers", recipe).toString(),
            buildEnvelope("test_nine_layers", recipe).toString().length());
        assertFalse(parsed.isSuccess(),
            "9-layer recipe (exceeds max=8) should be rejected by parser, but got success");
    }

    // ── 错误分支：layer 字段超范围 ────────────────────────────────────────────

    @Test
    void audioRecipeRejectsVolumeOutOfRange() {
        // 错误分支：layer.volume > AUDIO_VOLUME_MAX(=4.0) 应被拒绝。
        JsonObject recipe = new JsonObject();
        recipe.addProperty("id", "test_volume_oob");
        JsonArray layers = new JsonArray();
        layers.add(buildLayer("minecraft:block.note_block.harp", 999.0f, 1.0f, 0));
        recipe.add("layers", layers);
        recipe.addProperty("priority", 50);
        recipe.addProperty("attenuation", "world_3d");
        recipe.addProperty("category", "PLAYERS");
        recipe.addProperty("bus", "COMBAT");

        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(
            buildEnvelope("test_volume_oob", recipe).toString(),
            buildEnvelope("test_volume_oob", recipe).toString().length());
        assertFalse(parsed.isSuccess(),
            "layer.volume=999.0 (exceeds AUDIO_VOLUME_MAX=4.0) should be rejected, but got success");
    }

    @Test
    void audioRecipeRejectsPitchBelowMin() {
        // 错误分支：layer.pitch < AUDIO_PITCH_MIN(=0.1) 应被拒绝。
        JsonObject recipe = new JsonObject();
        recipe.addProperty("id", "test_pitch_below_min");
        JsonArray layers = new JsonArray();
        layers.add(buildLayer("minecraft:block.note_block.harp", 1.0f, 0.05f, 0));
        recipe.add("layers", layers);
        recipe.addProperty("priority", 50);
        recipe.addProperty("attenuation", "world_3d");
        recipe.addProperty("category", "PLAYERS");
        recipe.addProperty("bus", "COMBAT");

        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(
            buildEnvelope("test_pitch_below_min", recipe).toString(),
            buildEnvelope("test_pitch_below_min", recipe).toString().length());
        assertFalse(parsed.isSuccess(),
            "layer.pitch=0.05 (below AUDIO_PITCH_MIN=0.1) should be rejected, but got success");
    }

    @Test
    void audioRecipeRejectsPitchAboveMax() {
        // 错误分支：layer.pitch > AUDIO_PITCH_MAX(=2.0) 应被拒绝。
        JsonObject recipe = new JsonObject();
        recipe.addProperty("id", "test_pitch_above_max");
        JsonArray layers = new JsonArray();
        layers.add(buildLayer("minecraft:block.note_block.harp", 1.0f, 5.0f, 0));
        recipe.add("layers", layers);
        recipe.addProperty("priority", 50);
        recipe.addProperty("attenuation", "world_3d");
        recipe.addProperty("category", "PLAYERS");
        recipe.addProperty("bus", "COMBAT");

        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(
            buildEnvelope("test_pitch_above_max", recipe).toString(),
            buildEnvelope("test_pitch_above_max", recipe).toString().length());
        assertFalse(parsed.isSuccess(),
            "layer.pitch=5.0 (exceeds AUDIO_PITCH_MAX=2.0) should be rejected, but got success");
    }

    // ── 错误分支：缺失必填字段 ────────────────────────────────────────────────

    @Test
    void audioRecipeRejectsMissingId() {
        // 错误分支：recipe 缺失 "id" 字段应被拒绝。
        JsonObject recipe = new JsonObject();
        // 故意不添加 "id"
        JsonArray layers = new JsonArray();
        layers.add(buildLayer("minecraft:block.note_block.harp", 0.8f, 1.0f, 0));
        recipe.add("layers", layers);
        recipe.addProperty("priority", 50);
        recipe.addProperty("attenuation", "world_3d");
        recipe.addProperty("category", "PLAYERS");
        recipe.addProperty("bus", "COMBAT");

        // recipe_id 与 recipe.id 不一致（recipe.id 缺失），parser 应拒绝
        JsonObject envelope = new JsonObject();
        envelope.addProperty("v", 1);
        envelope.addProperty("recipe_id", "test_missing_id");
        envelope.addProperty("instance_id", 1);
        envelope.addProperty("volume_mul", 1.0);
        envelope.addProperty("pitch_shift", 0.0);
        envelope.add("recipe", recipe);

        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(
            envelope.toString(), envelope.toString().length());
        assertFalse(parsed.isSuccess(),
            "Recipe with missing 'id' field should be rejected by parser, but got success");
    }

    // ── 辅助方法 ─────────────────────────────────────────────────────────────

    /** 构造一个合法的 envelope wrapper，recipe 字段使用传入的 JsonObject。 */
    private static JsonObject buildEnvelope(String recipeId, JsonObject recipe) {
        JsonObject envelope = new JsonObject();
        envelope.addProperty("v", 1);
        envelope.addProperty("recipe_id", recipeId);
        envelope.addProperty("instance_id", 1);
        envelope.addProperty("volume_mul", 1.0);
        envelope.addProperty("pitch_shift", 0.0);
        envelope.add("recipe", recipe);
        return envelope;
    }

    /** 构造单个 layer JsonObject。 */
    private static JsonObject buildLayer(String sound, float volume, float pitch, int delayTicks) {
        JsonObject layer = new JsonObject();
        layer.addProperty("sound", sound);
        layer.addProperty("volume", volume);
        layer.addProperty("pitch", pitch);
        layer.addProperty("delay_ticks", delayTicks);
        return layer;
    }
}
