package com.bong.client.environment;

import net.minecraft.util.math.Vec3d;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertTrue;

class EnvironmentEffectRegistryTest {
    @Test
    void emitterRegistersBuiltInBehaviors() {
        EnvironmentEffectRegistry registry = registry();

        assertEquals(8, registry.behaviorCount());
    }

    @Test
    void activeNearPlayerWithinRadius() {
        EnvironmentEffectRegistry registry = registry();
        registry.onZoneStateUpdate(state(1, fog()));

        Vec3d player = new Vec3d(8.0, 70.0, 8.0);
        registry.tickFade(player, 80.0);

        assertEquals(1, registry.activeNearPlayer(player, 80.0).size());
    }

    @Test
    void activeNearPlayerOutsideRadiusEmpty() {
        EnvironmentEffectRegistry registry = registry();
        registry.onZoneStateUpdate(state(1, tornado()));

        Vec3d far = new Vec3d(400.0, 70.0, 400.0);
        registry.tickFade(far, 80.0);

        assertTrue(registry.activeNearPlayer(far, 80.0).isEmpty());
    }

    @Test
    void fadeInInterpolatesToOneOverFortyTicks() {
        EnvironmentEffectRegistry registry = registry();
        registry.onZoneStateUpdate(state(1, fog()));
        Vec3d player = new Vec3d(8.0, 70.0, 8.0);

        for (int i = 0; i < 40; i++) {
            registry.tickFade(player, 80.0);
        }

        ActiveEmitter active = registry.activeEmitters().iterator().next();
        assertEquals(1.0f, active.alpha(), 0.0001f);
    }

    @Test
    void fadeOutAfterLeavingRadius() {
        EnvironmentEffectRegistry registry = registry();
        registry.onZoneStateUpdate(state(1, fog()));
        Vec3d inside = new Vec3d(8.0, 70.0, 8.0);
        Vec3d outside = new Vec3d(400.0, 70.0, 400.0);

        for (int i = 0; i < 40; i++) {
            registry.tickFade(inside, 80.0);
        }
        registry.tickFade(outside, 80.0);

        ActiveEmitter active = registry.activeEmitters().iterator().next();
        assertTrue(active.alpha() < 1.0f);
        assertFalse(active.inRadius());
    }

    @Test
    void tornadoColumnZeroDensityStillCullsSafely() {
        EnvironmentEffect.TornadoColumn tornado =
            new EnvironmentEffect.TornadoColumn(0.0, 64.0, 0.0, 8.0, 48.0, 0.0);

        assertTrue(tornado.isNear(new Vec3d(2.0, 70.0, 2.0), 80.0));
        assertFalse(tornado.isNear(new Vec3d(200.0, 70.0, 200.0), 80.0));
    }

    @Test
    void fogVeilAabbCullingAtCorners() {
        EnvironmentEffect.FogVeil fog = fog();

        assertTrue(fog.contains(new Vec3d(0.0, 60.0, 0.0)));
        assertTrue(fog.contains(new Vec3d(16.0, 90.0, 16.0)));
        assertFalse(fog.contains(new Vec3d(16.1, 90.0, 16.0)));
    }

    @Test
    void effectDisappearsWhenZoneStateReplaced() {
        EnvironmentEffectRegistry registry = registry();
        registry.onZoneStateUpdate(state(1, fog()));
        Vec3d player = new Vec3d(8.0, 70.0, 8.0);
        for (int i = 0; i < 40; i++) {
            registry.tickFade(player, 80.0);
        }

        registry.onZoneStateUpdate(new ZoneEnvironmentState(1, "spawn", List.of(), 2));
        for (int i = 0; i < 40; i++) {
            registry.tickFade(player, 80.0);
        }

        assertTrue(registry.activeEmitters().isEmpty());
    }

    @Test
    void perfEightConcurrentEffectsInView() {
        EnvironmentEffectRegistry registry = registry();
        Vec3d player = new Vec3d(8.0, 70.0, 8.0);
        registry.onZoneStateUpdate(state(
            1,
            new EnvironmentEffect.TornadoColumn(8.0, 64.0, 8.0, 8.0, 48.0, 0.5),
            new EnvironmentEffect.LightningPillar(8.0, 64.0, 8.0, 4.0, 2.0),
            new EnvironmentEffect.AshFall(0.0, 60.0, 0.0, 16.0, 90.0, 16.0, 0.4),
            fog(),
            new EnvironmentEffect.DustDevil(8.0, 64.0, 8.0, 4.0, 24.0),
            new EnvironmentEffect.EmberDrift(0.0, 60.0, 0.0, 16.0, 90.0, 16.0, 0.3, 0.6),
            new EnvironmentEffect.HeatHaze(0.0, 60.0, 0.0, 16.0, 90.0, 16.0, 0.25),
            new EnvironmentEffect.SnowDrift(0.0, 60.0, 0.0, 16.0, 90.0, 16.0, 0.5, 0.5, 0.0, -0.25)
        ));

        registry.tickFade(player, 80.0);

        assertEquals(8, registry.activeNearPlayer(player, 80.0).size());
    }

    @Test
    void parserReadsZoneEnvironmentPayload() {
        EnvironmentEffectParser.ParseResult result = EnvironmentEffectParser.parse("""
            {
              "v": 1,
              "zone_id": "spawn",
              "generation": 7,
              "effects": [
                {
                  "kind": "fog_veil",
                  "aabb_min": [0.0, 60.0, 0.0],
                  "aabb_max": [16.0, 90.0, 16.0],
                  "tint_rgb": [120, 132, 148],
                  "density": 0.32
                }
              ]
            }
            """);

        assertTrue(result.ok(), result.error());
        assertEquals("spawn", result.state().zoneId());
        assertEquals(7L, result.state().generation());
        assertInstanceOf(EnvironmentEffect.FogVeil.class, result.state().effects().get(0));
    }

    private static EnvironmentEffectRegistry registry() {
        EnvironmentEffectRegistry registry = new EnvironmentEffectRegistry();
        registry.registerBuiltInBehaviors();
        return registry;
    }

    private static ZoneEnvironmentState state(long generation, EnvironmentEffect... effects) {
        return new ZoneEnvironmentState(1, "spawn", List.of(effects), generation);
    }

    private static EnvironmentEffect.FogVeil fog() {
        return new EnvironmentEffect.FogVeil(
            0.0, 60.0, 0.0,
            16.0, 90.0, 16.0,
            0x788494,
            0.32
        );
    }

    private static EnvironmentEffect.TornadoColumn tornado() {
        return new EnvironmentEffect.TornadoColumn(0.0, 64.0, 0.0, 8.0, 48.0, 0.5);
    }
}
