package com.bong.client.environment;

import net.minecraft.util.math.Vec3d;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class EnvironmentFogPlannerTest {
    @AfterEach
    void resetSink() {
        EnvironmentFogController.setSinkForTests(null);
        EnvironmentFogController.clear();
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
        String flag = "zone_env:" + emitter.key().hashCode();

        controller.update(List.of(emitter), new Vec3d(8.0, 70.0, 8.0));
        assertEquals(1, controller.activeLoopCountForTests());
        assertTrue(EnvironmentAudioLoopState.isActive(flag));

        controller.update(List.of(), new Vec3d(200.0, 70.0, 200.0));
        assertEquals(0, controller.activeLoopCountForTests());
        assertFalse(EnvironmentAudioLoopState.isActive(flag));
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
}
