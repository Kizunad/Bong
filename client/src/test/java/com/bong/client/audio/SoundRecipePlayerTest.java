package com.bong.client.audio;

import com.bong.client.network.AudioEventPayload;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicBoolean;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class SoundRecipePlayerTest {
    @Test
    void schedulesAllLayersWithVolumeAndPitchModifiers() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> false);

        player.play(playPayload(recipeWithoutLoop(), 0.5f, 0.0f));

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
    void loopReplaysWhileFlagStaysTrueAndStopsWhenFalse() {
        RecordingSink sink = new RecordingSink();
        AtomicBoolean active = new AtomicBoolean(true);
        SoundRecipePlayer player = new SoundRecipePlayer(sink, flag -> active.get());

        player.play(playPayload(recipeWithLoop(), 1.0f, 0.0f));
        assertEquals(1, player.activeLoopCountForTests());
        assertEquals(2, sink.played.size(), "initial play emits both layers");

        player.tick();
        assertEquals(2, sink.played.size(), "interval is 2 ticks, so first tick should not replay");
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
            AudioCategory.VOICE
        );
    }

    private static AudioRecipe recipeWithLoop() {
        return new AudioRecipe(
            "heartbeat_low_hp",
            recipeWithoutLoop().layers(),
            Optional.of(new AudioLoopConfig(2, "hp_below_30")),
            70,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.HOSTILE
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
    }
}
