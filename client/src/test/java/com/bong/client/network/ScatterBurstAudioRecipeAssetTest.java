package com.bong.client.network;

import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.visual.particle.ScatterBurstPlayer;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ScatterBurstAudioRecipeAssetTest {
    private static final Path RECIPE =
        Path.of("src/main/resources/assets/bong/audio_recipes/scatter_burst.json");

    @Test
    void scatterBurstAudioRecipeExistsAndParses() throws IOException {
        assertTrue(Files.isRegularFile(RECIPE), "散灵珠破裂音效资产 scatter_burst.json 必须提交");
        JsonObject recipe = JsonParser.parseString(Files.readString(RECIPE)).getAsJsonObject();
        assertEquals("scatter_burst", recipe.get("id").getAsString());
        assertEquals(2, recipe.getAsJsonArray("layers").size(), "散灵珠破裂应有 glass + breeze 两层音效");
        assertEquals(
            "minecraft:block.glass.break",
            recipe.getAsJsonArray("layers").get(0).getAsJsonObject().get("sound").getAsString()
        );
        assertEquals(
            "minecraft:entity.breeze.idle_air",
            recipe.getAsJsonArray("layers").get(1).getAsJsonObject().get("sound").getAsString()
        );

        AudioEventParseResult parsed =
            AudioEventEnvelope.parsePlay(envelope(recipe).toString(), envelope(recipe).toString().length());
        assertTrue(parsed.isSuccess(), "scatter_burst 应能通过 audio envelope 解析: " + parsed.errorMessage());
        AudioEventPayload.PlaySoundRecipe payload =
            assertInstanceOf(AudioEventPayload.PlaySoundRecipe.class, parsed.payload());
        AudioRecipe parsedRecipe = payload.recipe();
        assertEquals(AudioCategory.BLOCKS, parsedRecipe.category());
        assertEquals(AudioBus.ENVIRONMENT, parsedRecipe.bus());
    }

    @Test
    void scatterBurstAudioRecipeAssetMatchesProgrammaticRecipe() throws Exception {
        JsonObject recipe = JsonParser.parseString(Files.readString(RECIPE)).getAsJsonObject();
        assertRecipeEquals(programmaticRecipe(), parsedRecipe(recipe));
    }

    private static AudioRecipe parsedRecipe(JsonObject recipe) {
        String payload = envelope(recipe).toString();
        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(payload, payload.length());
        assertTrue(parsed.isSuccess(), "scatter_burst JSON 应能解析为 AudioRecipe: " + parsed.errorMessage());
        AudioEventPayload.PlaySoundRecipe play =
            assertInstanceOf(AudioEventPayload.PlaySoundRecipe.class, parsed.payload());
        return play.recipe();
    }

    private static AudioRecipe programmaticRecipe() throws Exception {
        Method method = ScatterBurstPlayer.class.getDeclaredMethod("audioRecipe");
        method.setAccessible(true);
        return (AudioRecipe) method.invoke(null);
    }

    private static void assertRecipeEquals(AudioRecipe expected, AudioRecipe actual) {
        assertEquals(expected.id(), actual.id(), "recipe id 必须与 ScatterBurstPlayer.audioRecipe 同步");
        assertEquals(expected.priority(), actual.priority(), "priority 必须与 ScatterBurstPlayer.audioRecipe 同步");
        assertEquals(expected.attenuation(), actual.attenuation(), "attenuation 必须与 ScatterBurstPlayer.audioRecipe 同步");
        assertEquals(expected.category(), actual.category(), "category 必须与 ScatterBurstPlayer.audioRecipe 同步");
        assertEquals(expected.bus(), actual.bus(), "bus 必须与 ScatterBurstPlayer.audioRecipe 同步");
        assertEquals(expected.loop(), actual.loop(), "loop 必须与 ScatterBurstPlayer.audioRecipe 同步");

        List<AudioLayer> expectedLayers = expected.layers();
        List<AudioLayer> actualLayers = actual.layers();
        assertEquals(expectedLayers.size(), actualLayers.size(), "layers 数量必须同步");
        for (int i = 0; i < expectedLayers.size(); i++) {
            AudioLayer expectedLayer = expectedLayers.get(i);
            AudioLayer actualLayer = actualLayers.get(i);
            assertEquals(expectedLayer.sound(), actualLayer.sound(), "layer " + i + " sound 必须同步");
            assertEquals(expectedLayer.volume(), actualLayer.volume(), 0.0001f, "layer " + i + " volume 必须同步");
            assertEquals(expectedLayer.pitch(), actualLayer.pitch(), 0.0001f, "layer " + i + " pitch 必须同步");
            assertEquals(expectedLayer.delayTicks(), actualLayer.delayTicks(), "layer " + i + " delay_ticks 必须同步");
        }
    }

    private static JsonObject envelope(JsonObject recipe) {
        JsonObject envelope = new JsonObject();
        envelope.addProperty("v", 1);
        envelope.addProperty("recipe_id", "scatter_burst");
        envelope.addProperty("instance_id", 1);
        envelope.addProperty("volume_mul", 1.0);
        envelope.addProperty("pitch_shift", 0.0);
        envelope.add("recipe", recipe);
        return envelope;
    }
}
