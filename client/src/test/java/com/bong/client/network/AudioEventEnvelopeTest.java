package com.bong.client.network;

import com.bong.client.audio.AudioAttenuation;
import com.bong.client.audio.AudioCategory;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class AudioEventEnvelopeTest {
    @Test
    void parsesValidPlayPayloadWithInlineRecipe() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-audio-play.json");

        AudioEventParseResult result = AudioEventEnvelope.parsePlay(json, jsonLen(json));

        assertTrue(result.isSuccess(), "valid audio play should parse: " + result.errorMessage());
        AudioEventPayload.PlaySoundRecipe payload = (AudioEventPayload.PlaySoundRecipe) result.payload();
        assertEquals("pill_consume", payload.recipeId());
        assertEquals(42L, payload.instanceId());
        assertEquals(1, payload.pos().orElseThrow().x());
        assertEquals(AudioAttenuation.PLAYER_LOCAL, payload.recipe().attenuation());
        assertEquals(AudioCategory.VOICE, payload.recipe().category());
        assertEquals(new Identifier("minecraft", "entity.generic.drink"), payload.recipe().layers().get(0).sound());
    }

    @Test
    void parsesValidStopPayload() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-audio-stop.json");

        AudioEventParseResult result = AudioEventEnvelope.parseStop(json, jsonLen(json));

        assertTrue(result.isSuccess());
        AudioEventPayload.StopSoundRecipe payload = (AudioEventPayload.StopSoundRecipe) result.payload();
        assertEquals(42L, payload.instanceId());
        assertEquals(10, payload.fadeOutTicks());
    }

    @Test
    void rejectsBadRecipeId() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-audio-bad-recipe-id.json");

        AudioEventParseResult result = AudioEventEnvelope.parsePlay(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("recipe_id"), result.errorMessage());
    }

    @Test
    void rejectsBadLayerPitch() throws IOException {
        String json = PayloadFixtureLoader.readText("invalid-audio-bad-layer-pitch.json");

        AudioEventParseResult result = AudioEventEnvelope.parsePlay(json, jsonLen(json));

        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("recipe"), result.errorMessage());
    }

    private static int jsonLen(String json) {
        return json.getBytes(StandardCharsets.UTF_8).length;
    }
}
