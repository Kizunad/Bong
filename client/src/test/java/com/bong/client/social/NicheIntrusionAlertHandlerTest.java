package com.bong.client.social;

import com.bong.client.network.AudioEventPayload;
import com.bong.client.network.AudioPlaybackBridge;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NicheIntrusionAlertHandlerTest {
    private final RecordingAudioBridge audio = new RecordingAudioBridge();

    @AfterEach
    void tearDown() {
        NicheGuardianStore.resetForTests();
        NicheIntrusionAlertHandler.resetAudioBridgeForTests();
    }

    @Test
    void guardianFatiguePlaysProgrammaticAudioRecipe() {
        NicheIntrusionAlertHandler.setAudioBridgeForTests(audio);

        NicheIntrusionAlertHandler.recordGuardianFatigue("puppet", 3);

        assertEquals(1, audio.played.size());
        AudioEventPayload.PlaySoundRecipe payload = audio.played.get(0);
        assertEquals("niche_guardian_fatigue", payload.recipeId());
        assertEquals("niche_guardian_fatigue", payload.recipe().id());
        assertEquals(72, payload.recipe().priority());
        assertEquals(1, payload.recipe().layers().size());
        assertEquals(
            new Identifier("minecraft", "block.grindstone.use"),
            payload.recipe().layers().get(0).sound()
        );
    }

    @Test
    void guardianBrokenPlaysProgrammaticAudioRecipe() {
        NicheIntrusionAlertHandler.setAudioBridgeForTests(audio);

        NicheIntrusionAlertHandler.recordGuardianBroken("puppet", "char:raider");

        assertEquals(1, audio.played.size());
        AudioEventPayload.PlaySoundRecipe payload = audio.played.get(0);
        assertEquals("niche_guardian_broken", payload.recipeId());
        assertEquals("niche_guardian_broken", payload.recipe().id());
        assertEquals(78, payload.recipe().priority());
        assertEquals(2, payload.recipe().layers().size());
        assertEquals(
            new Identifier("minecraft", "block.stone.break"),
            payload.recipe().layers().get(0).sound()
        );
        assertEquals(
            new Identifier("minecraft", "entity.wither.shoot"),
            payload.recipe().layers().get(1).sound()
        );
        assertTrue(NicheGuardianStore.guardianStatuses().get("puppet").broken());
    }

    private static final class RecordingAudioBridge implements AudioPlaybackBridge {
        final List<AudioEventPayload.PlaySoundRecipe> played = new ArrayList<>();

        @Override
        public boolean play(AudioEventPayload.PlaySoundRecipe payload) {
            played.add(payload);
            return true;
        }

        @Override
        public boolean stop(AudioEventPayload.StopSoundRecipe payload) {
            return true;
        }
    }
}
