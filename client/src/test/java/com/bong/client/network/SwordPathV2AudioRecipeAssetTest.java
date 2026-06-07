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
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
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
}
