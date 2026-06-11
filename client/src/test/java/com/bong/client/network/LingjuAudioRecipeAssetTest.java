package com.bong.client.network;

import com.bong.client.audio.AudioBus;
import com.bong.client.audio.AudioCategory;
import com.bong.client.audio.AudioRecipe;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertTrue;

class LingjuAudioRecipeAssetTest {
    private static final Path RECIPE =
        Path.of("src/main/resources/assets/bong/audio_recipes/lingju_activate.json");

    @Test
    void lingjuActivateAudioRecipeExistsAndParses() throws IOException {
        assertTrue(Files.isRegularFile(RECIPE), "聚灵激活音效资产 lingju_activate.json 必须提交");
        JsonObject recipe = JsonParser.parseString(Files.readString(RECIPE)).getAsJsonObject();
        assertEquals("lingju_activate", recipe.get("id").getAsString());
        assertEquals(2, recipe.getAsJsonArray("layers").size(), "聚灵激活应有 chime + cluster 两层音效");
        assertEquals(
            "minecraft:block.amethyst_block.chime",
            recipe.getAsJsonArray("layers").get(0).getAsJsonObject().get("sound").getAsString()
        );
        assertEquals(
            "minecraft:block.amethyst_cluster.step",
            recipe.getAsJsonArray("layers").get(1).getAsJsonObject().get("sound").getAsString()
        );

        AudioEventParseResult parsed = AudioEventEnvelope.parsePlay(envelope(recipe).toString(), envelope(recipe).toString().length());
        assertTrue(parsed.isSuccess(), "lingju_activate 应能通过 audio envelope 解析: " + parsed.errorMessage());
        AudioEventPayload.PlaySoundRecipe payload =
            assertInstanceOf(AudioEventPayload.PlaySoundRecipe.class, parsed.payload());
        AudioRecipe parsedRecipe = payload.recipe();
        assertEquals(AudioCategory.BLOCKS, parsedRecipe.category());
        assertEquals(AudioBus.ENVIRONMENT, parsedRecipe.bus());
    }

    private static JsonObject envelope(JsonObject recipe) {
        JsonObject envelope = new JsonObject();
        envelope.addProperty("v", 1);
        envelope.addProperty("recipe_id", "lingju_activate");
        envelope.addProperty("instance_id", 1);
        envelope.addProperty("volume_mul", 1.0);
        envelope.addProperty("pitch_shift", 0.0);
        envelope.add("recipe", recipe);
        return envelope;
    }
}
