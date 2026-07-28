package com.bong.client.environment;

import com.bong.client.audio.AudioScheduledSound;
import com.bong.client.audio.SoundRecipePlayer;
import com.bong.client.audio.SoundSink;
import net.minecraft.util.math.Vec3d;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class EnvironmentFogPlannerTest {
    @AfterEach
    void resetSink() {
        EnvironmentFogController.setSinkForTests(null);
        EnvironmentFogController.clear();
        EnvironmentAudioLoopState.clearOnDisconnect();
    }

    @Test
    void fogPlannerReturnsTintInsideFogVeilAabb() {
        ActiveEmitter emitter = active("a", 1, fog(0x788494, 0.5));

        EnvironmentFogCommand command = EnvironmentFogPlanner.plan(
            List.of(emitter),
            new Vec3d(8.0, 70.0, 8.0)
        );

        assertNotNull(command);
        assertEquals(0x788494, command.fogColorRgb());
    }

    @Test
    void fogPlannerReturnsDefaultOutsideAabb() {
        ActiveEmitter emitter = active("a", 1, fog(0x788494, 0.5));

        assertNull(EnvironmentFogPlanner.plan(
            List.of(emitter),
            new Vec3d(30.0, 70.0, 30.0)
        ));
    }

    @Test
    void skyPlannerBlendsTwoOverlappingZonesByGeneration() {
        ActiveEmitter oldFog = active("old", 1, fog(0x334455, 0.5));
        ActiveEmitter newFog = active("new", 3, fog(0xAA7744, 0.5));

        EnvironmentFogCommand command = EnvironmentFogPlanner.plan(
            List.of(oldFog, newFog),
            new Vec3d(8.0, 70.0, 8.0)
        );

        assertNotNull(command);
        assertEquals(0xAA7744, command.fogColorRgb());
    }

    @Test
    void fogControllerSinkReceivesPlannerOutput() {
        List<EnvironmentFogCommand> applied = new ArrayList<>();
        EnvironmentFogController.setSinkForTests(applied::add);
        EnvironmentFogController.update(
            List.of(active("a", 1, fog(0x788494, 0.5))),
            new Vec3d(8.0, 70.0, 8.0)
        );

        EnvironmentFogController.applyFog();

        assertEquals(1, applied.size());
        assertEquals(0x788494, applied.get(0).fogColorRgb());
    }

    @Test
    void audioLoopStartsAndStopsWhenPlayerEntersFogVeil() {
        EnvironmentAudioLoopState.clear();
        EnvironmentAudioController controller = new EnvironmentAudioController();
        ActiveEmitter emitter = active("fog-loop", 1, fog(0x788494, 0.5));
        String flag = EnvironmentAudioController.loopFlag(emitter.key());

        controller.update(List.of(emitter), new Vec3d(8.0, 70.0, 8.0));
        assertEquals(1, controller.activeLoopCountForTests());
        assertTrue(EnvironmentAudioLoopState.isActive(flag));

        controller.update(List.of(), new Vec3d(200.0, 70.0, 200.0));
        assertEquals(0, controller.activeLoopCountForTests());
        assertFalse(EnvironmentAudioLoopState.isActive(flag));
    }

    @Test
    void audioLoopClearOnDisconnectDropsDerivedFlags() {
        String oldFlag = EnvironmentAudioController.loopFlag("old-session-fog");
        String newFlag = EnvironmentAudioController.loopFlag("new-session-fog");
        EnvironmentAudioLoopState.activate(oldFlag);
        EnvironmentAudioLoopState.activate(newFlag);
        assertTrue(EnvironmentAudioLoopState.isActive(oldFlag), "前置：旧 session 派生 flag 必须存在");
        assertTrue(EnvironmentAudioLoopState.isActive(newFlag), "前置：第二个派生 flag 必须存在");

        EnvironmentAudioLoopState.clearOnDisconnect();

        assertFalse(EnvironmentAudioLoopState.isActive(oldFlag), "断线必须清空旧 session 的环境 loop flag");
        assertFalse(EnvironmentAudioLoopState.isActive(newFlag), "断线必须一次清空所有派生 flag");
        EnvironmentAudioLoopState.activate(newFlag);
        assertTrue(EnvironmentAudioLoopState.isActive(newFlag), "断线清理后新 session 必须能重新注册 flag");
    }

    @Test
    void audioControllerClearDropsAllLoopKeysBeforeRuntimeStopFailure() {
        RuntimeFailingSink sink = new RuntimeFailingSink();
        SoundRecipePlayer player = new SoundRecipePlayer(sink, EnvironmentAudioLoopState::isActive);
        EnvironmentAudioController controller = new EnvironmentAudioController(player);
        ActiveEmitter first = active("old-fog", 1, fog(0x788494, 0.5));
        ActiveEmitter second = active("old-fog-2", 1, fog(0x788494, 0.5));
        String firstFlag = EnvironmentAudioController.loopFlag(first.key());
        String secondFlag = EnvironmentAudioController.loopFlag(second.key());

        controller.update(List.of(first, second), new Vec3d(8.0, 70.0, 8.0));
        assertEquals(2, controller.activeLoopCountForTests(), "前置：旧 session 必须持有两个 environment loop key");

        IllegalStateException failure = assertThrows(IllegalStateException.class, controller::clear);

        assertEquals("stop failed", failure.getMessage(), "首个 RuntimeException 应交给父级中央清理隔离");
        assertEquals(2, sink.stopAttempts, "一个 stop 失败后仍必须尝试停止剩余旧 loop");
        assertEquals(0, controller.activeLoopCountForTests(), "stop 失败前必须先摘除全部旧 loop key");
        assertFalse(EnvironmentAudioLoopState.isActive(firstFlag), "stop 失败不得保留第一个旧 flag");
        assertFalse(EnvironmentAudioLoopState.isActive(secondFlag), "stop 失败不得保留第二个旧 flag");

        sink.failStops = false;
        controller.update(List.of(first), new Vec3d(8.0, 70.0, 8.0));
        assertEquals(1, controller.activeLoopCountForTests(), "新 session 同 key emitter 必须能重新启动 loop");
        assertTrue(EnvironmentAudioLoopState.isActive(firstFlag), "新 session 必须能重新激活同 key flag");
    }

    @Test
    void audioLoopFlagUsesFullKeyInsteadOfHashCode() {
        assertEquals("FB".hashCode(), "Ea".hashCode());
        assertNotEquals(
            EnvironmentAudioController.loopFlag("FB"),
            EnvironmentAudioController.loopFlag("Ea")
        );
    }

    @Test
    void defaultEmitterBehaviorHandlesNullAmbientEffect() {
        EmitterBehavior noop = (Vec3d playerPos, EnvironmentEffect ignored, float deltaTick) -> {
        };

        assertNull(noop.ambientLoopRecipe(null));
    }

    private static ActiveEmitter active(String key, long generation, EnvironmentEffect effect) {
        EmitterBehavior noop = (Vec3d playerPos, EnvironmentEffect ignored, float deltaTick) -> {
        };
        ActiveEmitter emitter = new ActiveEmitter(key, "spawn", effect, noop, generation);
        for (int i = 0; i < 40; i++) {
            emitter.advanceFade(true);
        }
        return emitter;
    }

    private static EnvironmentEffect.FogVeil fog(int tintRgb, double density) {
        return new EnvironmentEffect.FogVeil(
            0.0, 60.0, 0.0,
            16.0, 90.0, 16.0,
            tintRgb,
            density
        );
    }

    private static final class RuntimeFailingSink implements SoundSink {
        boolean failStops = true;
        int stopAttempts;

        @Override
        public boolean play(AudioScheduledSound sound) {
            return true;
        }

        @Override
        public void stop(long instanceId, int fadeOutTicks) {
            stopAttempts++;
            if (failStops) {
                throw new IllegalStateException("stop failed");
            }
        }
    }
}
