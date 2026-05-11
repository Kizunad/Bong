package com.bong.client.audio;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.environment.EnvironmentAudioLoopState;
import com.bong.client.hud.HudImmersionMode;
import com.bong.client.network.AudioEventPayload;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicBoolean;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

public class SoundRecipePlayerTest {
    @AfterEach
    void resetStores() {
        CombatHudStateStore.resetForTests();
        EnvironmentAudioLoopState.clear();
        HudImmersionMode.resetForTests();
    }

    @Test
    void schedulesAllLayersWithVolumeAndPitchModifiers() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false);

        player.play(playPayload(recipeWithoutLoop(), 0.5f, 0.0f));
        player.tick();

        assertEquals(2, sink.played.size());
        AudioScheduledSound first = sink.played.get(0);
        assertEquals(new Identifier("minecraft", "entity.generic.drink"), first.sound());
        assertEquals(0.2f, first.volume(), 0.0001f);
        assertEquals(1.0f, first.pitch(), 0.0001f);
        assertEquals(0, first.delayTicks());
        AudioScheduledSound second = sink.played.get(1);
        assertEquals(5, second.delayTicks());
    }

    @Test
    void preservesAudioWorldLowPitchFloor() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false);

        player.play(playPayload(lowPitchRecipe(), 1.0f, -1.0f));
        player.tick();

        assertEquals(1, sink.played.size());
        assertEquals(0.1f, sink.played.get(0).pitch(), 0.0001f);
    }

    @Test
    void loopReplaysWhileFlagStaysTrueAndStopsWhenFalse() {
        RecordingSink sink = new RecordingSink();
        AtomicBoolean active = new AtomicBoolean(true);
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> active.get());

        player.play(playPayload(recipeWithLoop(), 1.0f, 0.0f));
        assertEquals(1, player.activeLoopCountForTests());
        assertEquals(0, sink.played.size(), "play queues until end-of-tick drain");

        player.tick();
        assertEquals(2, sink.played.size(), "initial queued play emits both layers");
        player.tick();
        assertEquals(4, sink.played.size(), "second tick should replay both layers");

        active.set(false);
        player.tick();
        assertEquals(0, player.activeLoopCountForTests(), "false flag should remove loop");
    }

    @Test
    void stopRemovesLoopAndCallsSinkStop() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> true);
        player.play(playPayload(recipeWithLoop(), 1.0f, 0.0f));

        player.stop(new AudioEventPayload.StopSoundRecipe(42, 10));

        assertEquals(0, player.activeLoopCountForTests());
        assertEquals(42L, sink.stoppedInstanceId);
        assertEquals(10, sink.stoppedFadeOutTicks);
    }

    @Test
    void payloadFlagCanOwnLoopLifetimeUntilStop() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, EnvironmentAudioLoopState::isActive);

        player.play(playPayloadWithFlag(recipeWithLoop(), "fauna_fuya_pressure:42"));
        assertEquals(1, player.activeLoopCountForTests());

        player.tick();
        assertEquals(2, sink.played.size(), "owned flag should keep initial loop play alive");
        player.tick();
        assertEquals(4, sink.played.size(), "owned flag should keep replaying loop");

        player.stop(new AudioEventPayload.StopSoundRecipe(42, 10));

        assertEquals(0, player.activeLoopCountForTests());
        assertFalse(EnvironmentAudioLoopState.isActive("fauna_fuya_pressure:42"));
    }

    @Test
    void topNQueueKeepsThreeOneShotsAndOneLoopPerTick() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> true);

        player.play(playPayload(recipe("low", 10, Optional.empty()), 1.0f, 0.0f));
        player.play(playPayload(recipe("mid", 50, Optional.empty()), 1.0f, 0.0f));
        player.play(playPayload(recipe("high", 90, Optional.empty()), 1.0f, 0.0f));
        player.play(playPayload(recipe("higher", 95, Optional.empty()), 1.0f, 0.0f));
        player.play(playPayload(recipe("loop", 5, Optional.of(new AudioLoopConfig(20, "hp_below_30"))), 1.0f, 0.0f));

        player.tick();

        assertEquals(8, sink.played.size(), "four recipes with two layers each should play");
        assertEquals(0, sink.countRecipe("low"), "lowest one-shot should be dropped");
        assertEquals(2, sink.countRecipe("loop"), "one loop slot should be retained");
    }

    @Test
    void highPriorityPreemptsLowerSameCategoryLoop() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> true);
        player.play(playPayload(recipe("heartbeat_low_hp", 70, Optional.of(new AudioLoopConfig(20, "hp_below_30"))), 1.0f, 0.0f));
        player.tick();

        player.play(playPayload(recipe("tribulation_wave_impact", 98, Optional.empty()), 1.0f, 0.0f));

        assertEquals(0, player.activeLoopCountForTests());
        assertEquals(42L, sink.stoppedInstanceId);
    }

    @Test
    void ambientVolumeDucksWhileCombatHudIsActive() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false);
        CombatHudStateStore.replace(CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none()));

        player.play(playPayload(ambientRecipe(), 1.0f, 0.0f));
        player.tick();

        assertEquals(2, sink.played.size());
        assertEquals(0.393f, sink.played.get(0).volume(), 0.001f);
    }

    @Test
    void ambientDuckingReachesCombatTargetOverTwoSeconds() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false);
        CombatHudStateStore.replace(CombatHudState.create(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none()));

        for (int i = 0; i < 40; i++) {
            player.tick();
        }
        player.play(playPayload(ambientRecipe(), 1.0f, 0.0f));
        player.tick();

        assertEquals(2, sink.played.size());
        assertEquals(0.12f, sink.played.get(0).volume(), 0.0001f);
    }

    @Test
    void busMixerKeepsCombatVolumeIndependentFromEnvironment() {
        RecordingSink sink = new RecordingSink();
        AudioBusMixer mixer = new AudioBusMixer();
        mixer.setVolume(AudioBus.COMBAT, 0.25f);
        mixer.setVolume(AudioBus.ENVIRONMENT, 1.0f);
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false, mixer, new AudioTelemetry());

        player.play(playPayload(recipe("combat_hit", 50, Optional.empty()), 1.0f, 0.0f));
        player.tick();

        assertEquals(2, sink.played.size());
        assertEquals(0.1f, sink.played.get(0).volume(), 0.0001f);
        assertEquals(AudioBus.COMBAT, recipe("combat_hit", 50, Optional.empty()).bus());
    }

    @Test
    void tribulationDucksEnvironmentBusOnly() {
        RecordingSink sink = new RecordingSink();
        AudioBusMixer mixer = new AudioBusMixer();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false, mixer, new AudioTelemetry());
        player.setMusicState(MusicStateMachine.State.TRIBULATION);

        player.play(playPayload(ambientRecipe(), 1.0f, 0.0f));
        player.tick();

        assertEquals(2, sink.played.size());
        assertEquals(0.12f, sink.played.get(0).volume(), 0.0001f);
    }

    @Test
    void immersiveModeMutesUiBusUntilRestoreWindow() {
        RecordingSink sink = new RecordingSink();
        AudioBusMixer mixer = new AudioBusMixer();
        HudImmersionMode.setManualImmersive(true, 0L);
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false, mixer, new AudioTelemetry());

        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.tick();
        assertEquals(0.0f, sink.played.get(0).volume(), 0.0001f);

        mixer.restoreUiForTicks(5);
        player.play(playPayload(recipeWithoutLoop(), 1.0f, 0.0f));
        player.tick();
        assertEquals(0.4f, sink.played.get(2).volume(), 0.0001f);
    }

    @Test
    void telemetryFlagsRecipeOverplayWindow() {
        AudioTelemetry telemetry = new AudioTelemetry(1_000L, 2);

        telemetry.record("hit_light", 1_000L);
        telemetry.record("hit_light", 1_100L);
        telemetry.record("hit_light", 1_200L);

        assertEquals(true, telemetry.isOverThreshold("hit_light", 1_200L));
        assertEquals(false, telemetry.isOverThreshold("hit_light", 2_500L));
    }

    private static AudioEventPayload.PlaySoundRecipe playPayload(AudioRecipe recipe, float volumeMul, float pitchShift) {
        return new AudioEventPayload.PlaySoundRecipe(
            recipe.id(),
            42,
            Optional.of(new AudioPosition(1, 64, -2)),
            Optional.empty(),
            volumeMul,
            pitchShift,
            recipe
        );
    }

    private static AudioEventPayload.PlaySoundRecipe playPayloadWithFlag(AudioRecipe recipe, String flag) {
        return new AudioEventPayload.PlaySoundRecipe(
            recipe.id(),
            42,
            Optional.of(new AudioPosition(1, 64, -2)),
            Optional.of(flag),
            1.0f,
            0.0f,
            recipe
        );
    }

    private static AudioRecipe recipeWithoutLoop() {
        return new AudioRecipe(
            "pill_consume",
            List.of(
                new AudioLayer(new Identifier("minecraft", "entity.generic.drink"), 0.4f, 1.0f, 0),
                new AudioLayer(new Identifier("minecraft", "block.brewing_stand.brew"), 0.3f, 1.2f, 5)
            ),
            Optional.empty(),
            40,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.VOICE,
            AudioBus.UI
        );
    }

    private static AudioRecipe recipeWithLoop() {
        return new AudioRecipe(
            "heartbeat_low_hp",
            recipeWithoutLoop().layers(),
            Optional.of(new AudioLoopConfig(2, "hp_below_30")),
            70,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.HOSTILE,
            AudioBus.COMBAT
        );
    }

    private static AudioRecipe lowPitchRecipe() {
        return new AudioRecipe(
            "ambient_north_wastes",
            List.of(new AudioLayer(new Identifier("minecraft", "weather.rain"), 0.08f, 0.1f, 0)),
            Optional.empty(),
            24,
            AudioAttenuation.ZONE_BROADCAST,
            AudioCategory.AMBIENT,
            AudioBus.ENVIRONMENT
        );
    }

    private static AudioRecipe ambientRecipe() {
        return new AudioRecipe(
            "tribulation_thunder_distant",
            recipeWithoutLoop().layers(),
            Optional.empty(),
            95,
            AudioAttenuation.WORLD_3D,
            AudioCategory.AMBIENT,
            AudioBus.ENVIRONMENT
        );
    }

    private static AudioRecipe recipe(String id, int priority, Optional<AudioLoopConfig> loop) {
        return new AudioRecipe(
            id,
            List.of(
                new AudioLayer(new Identifier("minecraft", "audio_test/" + id + "_a"), 0.4f, 1.0f, 0),
                new AudioLayer(new Identifier("minecraft", "audio_test/" + id + "_b"), 0.3f, 1.2f, 5)
            ),
            loop,
            priority,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.HOSTILE,
            AudioBus.COMBAT
        );
    }

    private static final class RecordingSink implements SoundSink {
        final List<AudioScheduledSound> played = new ArrayList<>();
        long stoppedInstanceId = -1;
        int stoppedFadeOutTicks = -1;

        @Override
        public boolean play(AudioScheduledSound sound) {
            played.add(sound);
            return true;
        }

        @Override
        public void stop(long instanceId, int fadeOutTicks) {
            stoppedInstanceId = instanceId;
            stoppedFadeOutTicks = fadeOutTicks;
        }

        long countRecipe(String recipeId) {
            return played.stream()
                .filter(sound -> sound.sound().getPath().contains(recipeId))
                .count();
        }
    }
}
