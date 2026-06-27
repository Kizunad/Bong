package com.bong.client.visual.particle;

import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.network.AudioEventEnvelope;
import com.bong.client.network.AudioEventParseResult;
import com.bong.client.network.AudioEventPayload;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-tarkov-backpack-v1 P5 — 套包三类 audio recipe 资产对拍 + 差异化锁定。
 *
 * <p>每类 committed JSON 资产必须存在、可解析，且与 {@link PackOperationVfxPlayer#audioRecipe}
 * 程序化构造逐字段一致（防 JSON 与代码漂移）。并断言三类 recipe 互不相同（层数 / sound / pitch
 * 差异化，单方向 stub 撞红）。
 */
class PackOperationAudioRecipeAssetTest {
    private static final Path DIR = Path.of("src/main/resources/assets/bong/audio_recipes");

    @Test
    void unequipRecipeAssetMatchesProgrammatic() throws IOException {
        assertRecipeMatches("inventory_pack_unequip", PackOperationVfxPlayer.Kind.UNEQUIP);
    }

    @Test
    void equipRecipeAssetMatchesProgrammatic() throws IOException {
        assertRecipeMatches("inventory_pack_equip", PackOperationVfxPlayer.Kind.EQUIP);
    }

    @Test
    void stowRecipeAssetMatchesProgrammatic() throws IOException {
        assertRecipeMatches("inventory_pack_stow", PackOperationVfxPlayer.Kind.STOW);
    }

    @Test
    void unequipRecipeHasThreeLandingLayers() throws IOException {
        JsonObject recipe = recipeJson("inventory_pack_unequip");
        assertEquals(
            3,
            recipe.getAsJsonArray("layers").size(),
            "卸非空背包应有 3 层（落地 bamboo + 内含物 grass + 散落 gravel）"
        );
        assertEquals(
            "minecraft:block.bamboo.break",
            recipe.getAsJsonArray("layers").get(0).getAsJsonObject().get("sound").getAsString(),
            "卸包首层应为落地音 block.bamboo.break"
        );
    }

    /** 三类 audio recipe 必须差异化：id、首层 sound、层数集合互不相同。 */
    @Test
    void threeRecipesAreMutuallyDistinct() {
        AudioRecipe unequip = PackOperationVfxPlayer.audioRecipe(PackOperationVfxPlayer.Kind.UNEQUIP);
        AudioRecipe equip = PackOperationVfxPlayer.audioRecipe(PackOperationVfxPlayer.Kind.EQUIP);
        AudioRecipe stow = PackOperationVfxPlayer.audioRecipe(PackOperationVfxPlayer.Kind.STOW);

        // id 互不相同。
        assertNotEquals(unequip.id(), equip.id(), "卸/装 recipe id 必须不同");
        assertNotEquals(unequip.id(), stow.id(), "卸/拖入 recipe id 必须不同");
        assertNotEquals(equip.id(), stow.id(), "装/拖入 recipe id 必须不同");

        // 首层 vanilla sound 互不相同（音色差异化的核心证据）。
        assertNotEquals(
            unequip.layers().get(0).sound(), equip.layers().get(0).sound(),
            "卸/装 首层 sound 必须不同"
        );
        assertNotEquals(
            unequip.layers().get(0).sound(), stow.layers().get(0).sound(),
            "卸/拖入 首层 sound 必须不同"
        );
        assertNotEquals(
            equip.layers().get(0).sound(), stow.layers().get(0).sound(),
            "装/拖入 首层 sound 必须不同"
        );

        // 卸包层数（3）多于装/拖入（2）——强度递减直觉。
        assertEquals(3, unequip.layers().size(), "卸包 3 层");
        assertEquals(2, equip.layers().size(), "装包 2 层");
        assertEquals(2, stow.layers().size(), "拖入 2 层");
        assertTrue(
            unequip.layers().size() > stow.layers().size(),
            "卸包层数应多于拖入（反馈强度递减）"
        );
    }

    private void assertRecipeMatches(String id, PackOperationVfxPlayer.Kind kind) throws IOException {
        Path file = DIR.resolve(id + ".json");
        assertTrue(Files.isRegularFile(file), "套包音效资产 " + id + ".json 必须提交");
        JsonObject json = recipeJson(id);
        assertEquals(id, json.get("id").getAsString());

        AudioRecipe parsed = parseRecipe(json);
        AudioRecipe programmatic = PackOperationVfxPlayer.audioRecipe(kind);
        assertRecipeEquals(programmatic, parsed);
        assertEquals(AudioCategory.BLOCKS, parsed.category());
        assertEquals(AudioBus.ENVIRONMENT, parsed.bus());
    }

    private JsonObject recipeJson(String id) throws IOException {
        return JsonParser.parseString(Files.readString(DIR.resolve(id + ".json"))).getAsJsonObject();
    }

    private AudioRecipe parseRecipe(JsonObject recipe) {
        JsonObject envelope = new JsonObject();
        envelope.addProperty("v", 1);
        envelope.addProperty("recipe_id", recipe.get("id").getAsString());
        envelope.addProperty("instance_id", 1);
        envelope.addProperty("volume_mul", 1.0);
        envelope.addProperty("pitch_shift", 0.0);
        envelope.add("recipe", recipe);
        String payload = envelope.toString();
        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(payload, payload.length());
        assertTrue(parsed.isSuccess(), "音效资产应能解析为 AudioRecipe: " + parsed.errorMessage());
        AudioEventPayload.PlaySoundRecipe play =
            assertInstanceOf(AudioEventPayload.PlaySoundRecipe.class, parsed.payload());
        return play.recipe();
    }

    private void assertRecipeEquals(AudioRecipe expected, AudioRecipe actual) {
        assertEquals(expected.id(), actual.id(), "recipe id 必须与 PackOperationVfxPlayer.audioRecipe 同步");
        assertEquals(expected.priority(), actual.priority(), "priority 必须同步");
        assertEquals(expected.attenuation(), actual.attenuation(), "attenuation 必须同步");
        assertEquals(expected.category(), actual.category(), "category 必须同步");
        assertEquals(expected.bus(), actual.bus(), "bus 必须同步");

        List<AudioLayer> expectedLayers = expected.layers();
        List<AudioLayer> actualLayers = actual.layers();
        assertEquals(expectedLayers.size(), actualLayers.size(), "layers 数量必须同步");
        for (int i = 0; i < expectedLayers.size(); i++) {
            AudioLayer e = expectedLayers.get(i);
            AudioLayer a = actualLayers.get(i);
            assertEquals(e.sound(), a.sound(), "layer " + i + " sound 必须同步");
            assertEquals(e.volume(), a.volume(), 0.0001f, "layer " + i + " volume 必须同步");
            assertEquals(e.pitch(), a.pitch(), 0.0001f, "layer " + i + " pitch 必须同步");
            assertEquals(e.delayTicks(), a.delayTicks(), "layer " + i + " delay_ticks 必须同步");
        }
    }
}
