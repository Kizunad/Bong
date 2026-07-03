package com.bong.client.scroll;

import com.bong.client.audio.AudioAttenuation;
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
import java.util.function.Supplier;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-scroll-reading-v1 P2 — 卷轴阅读三条 audio recipe（开卷 / 翻页 / 合卷）资产对拍 +
 * 差异化锁定（照抄 {@code PackOperationAudioRecipeAssetTest} 模式）。
 *
 * <p>每类 committed JSON 资产必须存在、可解析，且与 {@link ScrollReadAudio} 程序化构造逐字段
 * 一致（防 JSON 与代码漂移）。并断言三类 recipe 互不相同（sound/pitch/priority 差异化）。
 */
class ScrollReadAudioRecipeAssetTest {
    private static final Path DIR = Path.of("src/main/resources/assets/bong/audio_recipes");

    @Test
    void openRecipeAssetMatchesProgrammatic() throws IOException {
        assertRecipeMatches("scroll_read_open", ScrollReadAudio::openRecipe);
    }

    @Test
    void pageTurnRecipeAssetMatchesProgrammatic() throws IOException {
        assertRecipeMatches("scroll_read_page_turn", ScrollReadAudio::pageTurnRecipe);
    }

    @Test
    void closeRecipeAssetMatchesProgrammatic() throws IOException {
        assertRecipeMatches("scroll_read_close", ScrollReadAudio::closeRecipe);
    }

    /** 三类 audio recipe 必须差异化：id、sound、pitch 互不相同（否则听感无法区分开卷/翻页/合卷）。 */
    @Test
    void threeRecipesAreMutuallyDistinct() {
        AudioRecipe open = ScrollReadAudio.openRecipe();
        AudioRecipe pageTurn = ScrollReadAudio.pageTurnRecipe();
        AudioRecipe close = ScrollReadAudio.closeRecipe();

        assertNotEquals(open.id(), pageTurn.id(), "开卷/翻页 recipe id 必须不同");
        assertNotEquals(open.id(), close.id(), "开卷/合卷 recipe id 必须不同");
        assertNotEquals(pageTurn.id(), close.id(), "翻页/合卷 recipe id 必须不同");

        // 开卷与合卷用不同 vanilla sound（page_turn vs put）。
        assertNotEquals(
            open.layers().get(0).sound(), close.layers().get(0).sound(),
            "开卷/合卷首层 sound 必须不同（page_turn vs put）"
        );
        // 开卷与翻页共用 page_turn sound，但 pitch 必须不同（否则听感无区分度）。
        assertEquals(
            open.layers().get(0).sound(), pageTurn.layers().get(0).sound(),
            "开卷/翻页共用 item.book.page_turn"
        );
        assertNotEquals(
            open.layers().get(0).pitch(), pageTurn.layers().get(0).pitch(), 0.0001f,
            "开卷/翻页 pitch 必须不同（听感区分「展卷」vs「翻页」）"
        );

        // 三者均为私人阅读反馈，不应广播给附近玩家。
        assertEquals(AudioAttenuation.PLAYER_LOCAL, open.attenuation());
        assertEquals(AudioAttenuation.PLAYER_LOCAL, pageTurn.attenuation());
        assertEquals(AudioAttenuation.PLAYER_LOCAL, close.attenuation());
    }

    private void assertRecipeMatches(String id, Supplier<AudioRecipe> programmaticSupplier) throws IOException {
        Path file = DIR.resolve(id + ".json");
        assertTrue(Files.isRegularFile(file), "卷轴阅读音效资产 " + id + ".json 必须提交");
        JsonObject json = recipeJson(id);
        assertEquals(id, json.get("id").getAsString());

        AudioRecipe parsed = parseRecipe(json);
        AudioRecipe programmatic = programmaticSupplier.get();
        assertRecipeEquals(programmatic, parsed);
        assertEquals(AudioCategory.VOICE, parsed.category());
        assertEquals(AudioBus.UI, parsed.bus());
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
        assertEquals(expected.id(), actual.id(), "recipe id 必须与 ScrollReadAudio 程序化构造同步");
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
