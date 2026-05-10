package com.bong.client.audio;

import com.bong.client.environment.EnvironmentAudioLoopState;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MusicStateMachineTest {
    @AfterEach
    void clearFlags() {
        EnvironmentAudioLoopState.clear();
    }

    @Test
    void musicStateTransitionsCrossfadePreviousLoop() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, EnvironmentAudioLoopState::isActive);
        MusicStateMachine machine = new MusicStateMachine(player);

        machine.apply(update("spawn", "ambient_spawn_plain", MusicStateMachine.State.AMBIENT, "ambient_flag"));
        player.tick();

        assertEquals(MusicStateMachine.State.AMBIENT, machine.currentStateForTests());
        assertTrue(EnvironmentAudioLoopState.isActive("ambient_flag"));
        assertEquals(1, player.activeLoopCountForTests());

        long ambientInstance = machine.activeInstanceIdForTests();
        machine.apply(update("spawn", "combat_music", MusicStateMachine.State.COMBAT, "combat_flag"));
        player.tick();

        assertEquals(MusicStateMachine.State.COMBAT, machine.currentStateForTests());
        assertFalse(EnvironmentAudioLoopState.isActive("ambient_flag"));
        assertTrue(EnvironmentAudioLoopState.isActive("combat_flag"));
        assertEquals(ambientInstance, sink.stoppedInstanceId);
        assertEquals(60, sink.stoppedFadeOutTicks);
    }

    @Test
    void tribulationOverridesCombatByPriorityResolver() {
        assertEquals(
            MusicStateMachine.State.TRIBULATION,
            MusicStateMachine.State.resolve(true, true, true, true)
        );
        assertEquals(
            MusicStateMachine.State.COMBAT,
            MusicStateMachine.State.resolve(false, true, true, true)
        );
        assertEquals(
            MusicStateMachine.State.TSY,
            MusicStateMachine.State.resolve(false, false, true, true)
        );
        assertEquals(
            MusicStateMachine.State.CULTIVATION,
            MusicStateMachine.State.resolve(false, false, false, true)
        );
    }

    @Test
    void stateParserUsesLocaleIndependentUppercase() {
        assertEquals(Optional.of(MusicStateMachine.State.COMBAT), MusicStateMachine.State.fromWire("combat"));
    }

    @Test
    void identicalUpdateDoesNotRestartLoop() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, EnvironmentAudioLoopState::isActive);
        MusicStateMachine machine = new MusicStateMachine(player);
        MusicStateMachine.AmbientZoneUpdate update =
            update("spawn", "ambient_spawn_plain", MusicStateMachine.State.AMBIENT, "ambient_flag");

        assertTrue(machine.apply(update));
        assertFalse(machine.apply(update));

        assertEquals(-1L, sink.stoppedInstanceId);
        assertEquals(1, player.activeLoopCountForTests());
    }

    @Test
    void sameRecipeIdWithChangedRecipeRestartsLoop() {
        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, EnvironmentAudioLoopState::isActive);
        MusicStateMachine machine = new MusicStateMachine(player);
        MusicStateMachine.AmbientZoneUpdate first =
            update("spawn", "ambient_spawn_plain", MusicStateMachine.State.AMBIENT, "ambient_flag");
        MusicStateMachine.AmbientZoneUpdate changedRecipe =
            update("spawn", "ambient_spawn_plain", MusicStateMachine.State.AMBIENT, "ambient_flag", 55);

        assertTrue(machine.apply(first));
        long firstInstance = machine.activeInstanceIdForTests();
        assertTrue(machine.apply(changedRecipe));

        assertEquals(firstInstance, sink.stoppedInstanceId);
    }

    private static MusicStateMachine.AmbientZoneUpdate update(
        String zone,
        String recipeId,
        MusicStateMachine.State state,
        String flag
    ) {
        return update(zone, recipeId, state, flag, 50);
    }

    private static MusicStateMachine.AmbientZoneUpdate update(
        String zone,
        String recipeId,
        MusicStateMachine.State state,
        String flag,
        int priority
    ) {
        return new MusicStateMachine.AmbientZoneUpdate(
            zone,
            recipeId,
            state,
            false,
            "summer",
            Optional.empty(),
            60,
            Optional.of(new AudioPosition(0, 64, 0)),
            1.0f,
            0.0f,
            recipe(recipeId, flag, priority)
        );
    }

    private static AudioRecipe recipe(String id, String flag, int priority) {
        return new AudioRecipe(
            id,
            List.of(new AudioLayer(new Identifier("minecraft", "ambient.cave"), 0.2f, 1.0f, 0)),
            Optional.of(new AudioLoopConfig(80, flag)),
            priority,
            AudioAttenuation.PLAYER_LOCAL,
            AudioCategory.AMBIENT
        );
    }

    private static final class RecordingSink implements SoundSink {
        final List<AudioScheduledSound> played = new ArrayList<>();
        long stoppedInstanceId = -1L;
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
