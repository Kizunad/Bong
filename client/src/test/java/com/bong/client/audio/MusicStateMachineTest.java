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
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
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
    void clearOnDisconnectDelegatesToFullBusinessClear() throws Exception {
        java.nio.file.Path workingDirectory = java.nio.file.Path.of("").toAbsolutePath().normalize();
        java.nio.file.Path clientRoot = java.nio.file.Files.isDirectory(workingDirectory.resolve("src"))
            ? workingDirectory
            : workingDirectory.resolve("client");
        String source = java.nio.file.Files.readString(clientRoot.resolve(
            "src/main/java/com/bong/client/audio/MusicStateMachine.java"
        ));

        assertTrue(
            source.contains("public static void clearOnDisconnect() {\n        INSTANCE.clear();\n    }"),
            "断线入口必须复用既有 clear()，保留 AMBIENT 复位与活动 transition 硬停语义"
        );
    }

    @Test
    void seasonModifierDisconnectClearTargetsReceiverInstance() {
        MusicStateMachine singleton = MusicStateMachine.instance();
        singleton.setSeasonModifier(com.bong.client.state.SeasonState.Phase.WINTER, 0.75);

        RecordingSink sink = new RecordingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, EnvironmentAudioLoopState::isActive);
        MusicStateMachine machine = new MusicStateMachine(player);
        machine.setSeasonModifier(com.bong.client.state.SeasonState.Phase.SUMMER_TO_WINTER, 0.4);

        machine.clearSeasonModifierOnDisconnect();

        assertEquals(com.bong.client.state.SeasonState.Phase.SUMMER, machine.seasonModifierForTests().phase(),
            "实例断线清理必须复位接收者自己的季节相位");
        assertEquals(0.0, machine.seasonModifierForTests().progress(), 1e-6,
            "实例断线清理必须复位接收者自己的季节进度");
        assertEquals(com.bong.client.state.SeasonState.Phase.WINTER, singleton.seasonModifierForTests().phase(),
            "清理注入实例不得误清全局 singleton");
        assertEquals(0.75, singleton.seasonModifierForTests().progress(), 1e-6,
            "清理注入实例不得改写全局 singleton 的季节进度");

        singleton.clearSeasonModifierForTests();
    }

    @Test
    void clearDropsActiveMusicBeforeSinkRuntimeFailureSoSameUpdateCanRestart() {
        RuntimeFailingSink sink = new RuntimeFailingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, EnvironmentAudioLoopState::isActive);
        MusicStateMachine machine = new MusicStateMachine(player);
        MusicStateMachine.AmbientZoneUpdate update =
            update("spawn", "ambient_spawn_plain", MusicStateMachine.State.COMBAT, "ambient_flag");

        assertTrue(machine.apply(update), "前置：旧 session 必须建立 active music");
        long oldInstanceId = machine.activeInstanceIdForTests();

        IllegalStateException failure = assertThrows(IllegalStateException.class, machine::clear);

        assertEquals("stop failed", failure.getMessage(), "RuntimeException 应交给中央 adjunct 隔离并记录");
        assertEquals(0L, machine.activeInstanceIdForTests(), "sink 失败前必须先摘除旧 active music");
        assertNull(machine.currentStateForTests(), "sink 失败不得保留旧 transition key");
        assertFalse(EnvironmentAudioLoopState.isActive("ambient_flag"), "sink 失败不得保留旧 music loop flag");
        assertEquals(1.0f, player.mixerForTests().effectiveVolume(AudioBus.ENVIRONMENT), 0.0001f,
            "sink 失败前必须先恢复默认 music bus 状态");

        sink.failStops = false;
        assertTrue(machine.apply(update), "新 session 的同 key update 必须重新启动，而非被旧 key 短路");
        assertTrue(machine.activeInstanceIdForTests() > oldInstanceId, "新 session 必须分配新的 music instance");
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

    @Test
    void updateRejectsInvalidWireFields() {
        AudioRecipe recipe = recipe("ambient_spawn_plain", "ambient_flag", 50);

        assertThrows(IllegalArgumentException.class, () -> new MusicStateMachine.AmbientZoneUpdate(
            "spawn",
            "ambient_spawn_plain",
            MusicStateMachine.State.AMBIENT,
            false,
            "summer",
            Optional.of("abyss"),
            60,
            Optional.of(new AudioPosition(0, 64, 0)),
            1.0f,
            0.0f,
            recipe
        ));
        assertThrows(IllegalArgumentException.class, () -> new MusicStateMachine.AmbientZoneUpdate(
            "spawn",
            "ambient_spawn_plain",
            MusicStateMachine.State.AMBIENT,
            false,
            "summer",
            Optional.empty(),
            -1,
            Optional.of(new AudioPosition(0, 64, 0)),
            1.0f,
            0.0f,
            recipe
        ));
        assertThrows(IllegalArgumentException.class, () -> new MusicStateMachine.AmbientZoneUpdate(
            "spawn",
            "ambient_spawn_plain",
            MusicStateMachine.State.AMBIENT,
            false,
            "summer",
            Optional.empty(),
            60,
            Optional.of(new AudioPosition(30_000_001, 64, 0)),
            1.0f,
            0.0f,
            recipe
        ));
        assertThrows(IllegalArgumentException.class, () -> new MusicStateMachine.AmbientZoneUpdate(
            "spawn",
            "ambient_spawn_plain",
            MusicStateMachine.State.AMBIENT,
            false,
            "summer",
            Optional.empty(),
            60,
            Optional.of(new AudioPosition(0, 64, 0)),
            4.5f,
            0.0f,
            recipe
        ));
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
            AudioCategory.AMBIENT,
            AudioBus.ENVIRONMENT
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

    private static final class RuntimeFailingSink implements SoundSink {
        boolean failStops = true;

        @Override
        public boolean play(AudioScheduledSound sound) {
            return true;
        }

        @Override
        public void stop(long instanceId, int fadeOutTicks) {
            if (failStops) {
                throw new IllegalStateException("stop failed");
            }
        }
    }
}
