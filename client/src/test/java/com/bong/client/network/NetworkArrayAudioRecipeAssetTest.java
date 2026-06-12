package com.bong.client.network;

import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioLayer;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.visual.particle.NetworkArrayFormPlayer;
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

class NetworkArrayAudioRecipeAssetTest {
    private static final Path RECIPE_ROOT =
        Path.of("src/main/resources/assets/bong/audio_recipes");

    @Test
    void networkArrayFormAudioRecipeExistsAndParses() throws Exception {
        JsonObject recipe = readRecipe("network_array_form");
        assertEquals("network_array_form", recipe.get("id").getAsString());
        assertLayer(recipe, 0, "minecraft:block.beacon.activate", 0.5f, 1.3f, 0);
        assertLayer(recipe, 1, "minecraft:block.amethyst_block.chime", 0.3f, 1.0f, 6);

        AudioRecipe parsed = parsedRecipe(recipe);
        assertEquals(AudioCategory.BLOCKS, parsed.category());
        assertEquals(AudioBus.ENVIRONMENT, parsed.bus());
        assertRecipeEquals(programmaticRecipe(NetworkArrayFormPlayer.Kind.FORM), parsed);
    }

    @Test
    void networkArrayBreakAudioRecipeExistsAndParses() throws Exception {
        JsonObject recipe = readRecipe("network_array_break");
        assertEquals("network_array_break", recipe.get("id").getAsString());
        assertLayer(recipe, 0, "minecraft:block.beacon.deactivate", 0.5f, 0.9f, 0);
        assertLayer(recipe, 1, "minecraft:block.glass.break", 0.4f, 0.7f, 2);

        AudioRecipe parsed = parsedRecipe(recipe);
        assertEquals(AudioCategory.BLOCKS, parsed.category());
        assertEquals(AudioBus.ENVIRONMENT, parsed.bus());
        assertRecipeEquals(programmaticRecipe(NetworkArrayFormPlayer.Kind.BREAK), parsed);
    }

    private static JsonObject readRecipe(String id) throws IOException {
        Path recipe = RECIPE_ROOT.resolve(id + ".json");
        assertTrue(Files.isRegularFile(recipe), id + " 音效资产必须提交");
        JsonObject obj = JsonParser.parseString(Files.readString(recipe)).getAsJsonObject();
        assertEquals(2, obj.getAsJsonArray("layers").size(), id + " 应有两层音效");
        return obj;
    }

    private static AudioRecipe parsedRecipe(JsonObject recipe) {
        String payload = envelope(recipe).toString();
        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(payload, payload.length());
        assertTrue(parsed.isSuccess(), recipe.get("id").getAsString() + " 应能通过 audio envelope 解析: "
            + parsed.errorMessage());
        AudioEventPayload.PlaySoundRecipe play =
            assertInstanceOf(AudioEventPayload.PlaySoundRecipe.class, parsed.payload());
        return play.recipe();
    }

    private static AudioRecipe programmaticRecipe(NetworkArrayFormPlayer.Kind kind) throws Exception {
        Method method = NetworkArrayFormPlayer.class.getDeclaredMethod("audioRecipe", NetworkArrayFormPlayer.Kind.class);
        method.setAccessible(true);
        return (AudioRecipe) method.invoke(null, kind);
    }

    private static void assertLayer(
        JsonObject recipe,
        int index,
        String sound,
        float volume,
        float pitch,
        int delayTicks
    ) {
        JsonObject layer = recipe.getAsJsonArray("layers").get(index).getAsJsonObject();
        assertEquals(sound, layer.get("sound").getAsString(), "layer " + index + " sound 必须符合 P3 spec");
        assertEquals(volume, layer.get("volume").getAsFloat(), 0.0001f, "layer " + index + " volume 必须符合 P3 spec");
        assertEquals(pitch, layer.get("pitch").getAsFloat(), 0.0001f, "layer " + index + " pitch 必须符合 P3 spec");
        assertEquals(delayTicks, layer.get("delay_ticks").getAsInt(), "layer " + index + " delay_ticks 必须符合 P3 spec");
    }

    private static void assertRecipeEquals(AudioRecipe expected, AudioRecipe actual) {
        assertEquals(expected.id(), actual.id(), "recipe id 必须与 NetworkArrayFormPlayer.audioRecipe 同步");
        assertEquals(expected.priority(), actual.priority(), "priority 必须同步");
        assertEquals(expected.attenuation(), actual.attenuation(), "attenuation 必须同步");
        assertEquals(expected.category(), actual.category(), "category 必须同步");
        assertEquals(expected.bus(), actual.bus(), "bus 必须同步");
        assertEquals(expected.loop(), actual.loop(), "loop 必须同步");

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
        envelope.addProperty("recipe_id", recipe.get("id").getAsString());
        envelope.addProperty("instance_id", 1);
        envelope.addProperty("volume_mul", 1.0);
        envelope.addProperty("pitch_shift", 0.0);
        envelope.add("recipe", recipe);
        return envelope;
    }
}
