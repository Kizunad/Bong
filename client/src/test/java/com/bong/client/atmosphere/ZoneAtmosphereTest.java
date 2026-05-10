package com.bong.client.atmosphere;

import com.bong.client.environment.EnvironmentFogCommand;
import com.bong.client.state.SeasonState;
import com.bong.client.state.ZoneState;
import net.minecraft.util.math.Vec3d;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ZoneAtmosphereTest {
    @AfterEach
    void resetRenderer() {
        ZoneAtmosphereRenderer.resetForTests();
    }

    @Test
    void profile_loads_from_json() {
        ZoneAtmosphereProfile profile = ZoneAtmosphereProfileRegistry.loadDefault().forZone("spawn_plain");

        assertEquals(0xB0C4DE, profile.fogColorRgb());
        assertEquals(0.15, profile.fogDensity(), 0.0001);
        assertEquals("ambient_spawn_plain", profile.ambientRecipeId());
        assertEquals(ZoneAtmosphereProfile.TransitionFx.NONE, profile.entryTransitionFx());
    }

    @Test
    void fog_overlays_realm_vision() {
        ZoneAtmosphereProfileRegistry registry = ZoneAtmosphereProfileRegistry.loadDefault();
        ZoneAtmosphereCommand atmosphere = ZoneAtmospherePlanner.plan(
            registry,
            ZoneAtmosphereContext.of(ZoneState.create("qingyun_peaks", "Qingyun", 0.8, 2, 1L), null),
            1L
        );
        EnvironmentFogCommand realmFog = new EnvironmentFogCommand(18.0, 80.0, 0x203040, 0x304050, 0.6);

        EnvironmentFogCommand merged = ZoneAtmosphereRenderer.mergeFogCommands(
            realmFog,
            new EnvironmentFogCommand(
                atmosphere.fogStart(),
                atmosphere.fogEnd(),
                atmosphere.fogColorRgb(),
                atmosphere.skyTintRgb(),
                atmosphere.fogDensity()
            )
        );

        assertEquals(0.6, merged.density(), 0.0001);
        assertNotEquals(realmFog.fogColorRgb(), merged.fogColorRgb());
        assertNotEquals(atmosphere.fogColorRgb(), merged.fogColorRgb());
    }

    @Test
    void boundary_lerp_150_blocks() {
        ZoneAtmosphereProfileRegistry registry = ZoneAtmosphereProfileRegistry.loadDefault();
        ZoneAtmosphereProfile spawn = registry.forZone("spawn_plain");
        ZoneAtmosphereProfile qingyun = registry.forZone("qingyun_peaks");

        ZoneAtmosphereProfile midpoint = ZoneBoundaryTransition.blend(
            spawn,
            qingyun,
            ZoneBoundaryTransition.progress(75.0)
        );

        assertEquals(0.5, ZoneBoundaryTransition.progress(75.0), 0.0001);
        assertEquals(0x98AABF, midpoint.fogColorRgb());
        assertEquals(0.225, midpoint.fogDensity(), 0.0001);
        assertEquals(2, midpoint.ambientParticles().size());
    }

    @Test
    void hot_reload_updates_fog() {
        ZoneAtmosphereRenderer.reloadProfilesForTests(Map.of(
            "spawn_plain",
            """
            {
              "zone_id":"spawn_plain",
              "fog_color":"#112233",
              "fog_density":0.77,
              "ambient_particle":{"type":"cloud256_dust","tint":"#445566","density":0.2},
              "sky_tint":"#223344",
              "entry_transition_fx":"FADE",
              "ambient_recipe_id":"ambient_spawn_plain"
            }
            """
        ));
        ZoneAtmosphereProfileRegistry reloaded = ZoneAtmosphereProfileRegistry.fromJson(Map.of(
            "spawn_plain",
            """
            {
              "zone_id":"spawn_plain",
              "fog_color":"#112233",
              "fog_density":0.77,
              "ambient_particle":{"type":"cloud256_dust","tint":"#445566","density":0.2},
              "sky_tint":"#223344",
              "entry_transition_fx":"FADE",
              "ambient_recipe_id":"ambient_spawn_plain"
            }
            """
        ));

        assertEquals(0x112233, reloaded.forZone("spawn_plain").fogColorRgb());
        assertEquals(0.77, reloaded.forZone("spawn_plain").fogDensity(), 0.0001);
    }

    @Test
    void parser_rejects_non_array_ambient_particles() {
        ZoneAtmosphereProfileParser.ParseResult result = ZoneAtmosphereProfileParser.parse(
            """
            {
              "zone_id":"spawn_plain",
              "fog_color":"#112233",
              "fog_density":0.77,
              "ambient_particles":{"type":"cloud256_dust","tint":"#445566","density":0.2},
              "sky_tint":"#223344",
              "entry_transition_fx":"FADE",
              "ambient_recipe_id":"ambient_spawn_plain"
            }
            """,
            "spawn_plain"
        );

        assertFalse(result.ok());
        assertTrue(result.error().contains("ambient_particles must be an array"));
    }

    @Test
    void all_zones_have_profile() {
        ZoneAtmosphereProfileRegistry registry = ZoneAtmosphereProfileRegistry.loadDefault();

        for (String zoneId : ZoneAtmosphereProfileRegistry.REQUIRED_PROFILE_IDS) {
            assertTrue(registry.hasProfile(zoneId), "missing atmosphere profile for " + zoneId);
        }
    }

    @Test
    void dead_zone_desaturation_50pct() {
        ZoneAtmosphereCommand command = commandFor(
            ZoneState.create("blood_valley", "Blood Valley", 0.0, 5, "collapsed", 10L),
            null
        );

        assertEquals(0.5, command.desaturation(), 0.0001);
        assertEquals(1.0, command.fogDensity(), 0.0001);
        assertEquals(150.0, command.fogEnd(), 0.0001);
        assertTrue(command.deadZoneVisual());
    }

    @Test
    void negative_qi_vignette_intensity() {
        ZoneAtmosphereCommand command = commandFor(
            ZoneState.create("blood_valley", "Blood Valley", -2.0, 5, 10L),
            null
        );

        assertEquals(0.6, command.vignetteIntensity(), 0.0001);
        assertTrue(command.negativeZoneVisual());
        assertTrue(command.distortionIntensity() > 0.0);
    }

    @Test
    void ash_footprint_on_step() {
        AshFootprintTracker tracker = new AshFootprintTracker();
        ZoneAtmosphereCommand dead = commandFor(
            ZoneState.create("blood_valley", "Blood Valley", 0.0, 5, "collapsed", 10L),
            null
        );

        List<AshFootprintTracker.FootprintCommand> commands =
            tracker.onEntityStep(7L, new Vec3d(1.0, 64.0, 1.0), 20L, dead);

        assertEquals(2, commands.size());
        assertEquals("ash_burst", commands.get(0).kind());
        assertEquals("ash_footprint_decal", commands.get(1).kind());
    }

    @Test
    void ash_footprint_throttles_by_distance_or_interval() {
        ZoneAtmosphereCommand dead = commandFor(
            ZoneState.create("blood_valley", "Blood Valley", 0.0, 5, "collapsed", 10L),
            null
        );

        AshFootprintTracker distanceTracker = new AshFootprintTracker();
        assertFalse(distanceTracker.onEntityStep(7L, new Vec3d(1.0, 64.0, 1.0), 20L, dead).isEmpty());
        assertTrue(distanceTracker.onEntityStep(7L, new Vec3d(1.1, 64.0, 1.1), 40L, dead).isEmpty());

        AshFootprintTracker intervalTracker = new AshFootprintTracker();
        assertFalse(intervalTracker.onEntityStep(8L, new Vec3d(1.0, 64.0, 1.0), 20L, dead).isEmpty());
        assertTrue(intervalTracker.onEntityStep(8L, new Vec3d(3.0, 64.0, 3.0), 22L, dead).isEmpty());
        assertFalse(intervalTracker.onEntityStep(8L, new Vec3d(3.0, 64.0, 3.0), 50L, dead).isEmpty());
    }

    @Test
    void tsy_fog_by_tier() {
        ZoneAtmosphereCommand shallow = commandForTsyTier(2);
        ZoneAtmosphereCommand middle = commandForTsyTier(5);
        ZoneAtmosphereCommand deep = commandForTsyTier(7);

        assertEquals(50.0, shallow.fogEnd(), 0.0001);
        assertEquals(20.0, middle.fogEnd(), 0.0001);
        assertEquals(8.0, deep.fogEnd(), 0.0001);
        assertEquals(0x101015, deep.fogColorRgb());
    }

    @Test
    void tsy_deep_breathing_scale() {
        ZoneAtmosphereCommand deep = commandForTsyTier(7);

        assertEquals(0.005, deep.breathingScale(), 0.0001);
    }

    @Test
    void collapse_visual_sequence_timing() {
        ZoneAtmosphereProfileRegistry registry = ZoneAtmosphereProfileRegistry.loadDefault();
        ZoneAtmosphereContext context = ZoneAtmosphereContext
            .of(ZoneState.create("tsy_lingxu", "TSY", 0.4, 5, 10L), null)
            .withTsyTier(7)
            .withCollapse(200, 1200);

        ZoneAtmosphereCommand command = ZoneAtmospherePlanner.plan(registry, context, 1L);

        assertEquals(0x000000, command.fogColorRgb());
        assertEquals(0.5, command.cameraShakeIntensity(), 0.0001);
        assertTrue(command.hardClipVoid());
    }

    @Test
    void collapse_vignette_renders_without_negative_qi() {
        ZoneAtmosphereCommand command = ZoneAtmospherePlanner.plan(
            ZoneAtmosphereProfileRegistry.loadDefault(),
            ZoneAtmosphereContext
                .of(ZoneState.create("tsy_lingxu", "TSY", 0.4, 5, 10L), null)
                .withTsyTier(7)
                .withCollapse(400, 1200),
            1L
        );
        List<com.bong.client.hud.HudRenderCommand> commands = new ArrayList<>();

        ZoneAtmosphereHudPlanner.append(commands, command);

        assertTrue(commands.stream().anyMatch(com.bong.client.hud.HudRenderCommand::isEdgeVignette));
    }

    @Test
    void summer_reduces_fog_density() {
        ZoneAtmosphereCommand base = commandFor(
            ZoneState.create("qingyun_peaks", "Qingyun", 0.8, 2, 10L),
            null
        );
        ZoneAtmosphereCommand summer = commandFor(
            ZoneState.create("qingyun_peaks", "Qingyun", 0.8, 2, 10L),
            new SeasonState(SeasonState.Phase.SUMMER, 0L, 100L, 0L)
        );

        assertTrue(summer.fogDensity() < base.fogDensity());
    }

    @Test
    void dead_zone_ignores_season() {
        ZoneState dead = ZoneState.create("north_wastes", "North", 0.0, 5, "collapsed", 10L);
        ZoneAtmosphereCommand summer = commandFor(dead, new SeasonState(SeasonState.Phase.SUMMER, 0L, 100L, 0L));
        ZoneAtmosphereCommand winter = commandFor(dead, new SeasonState(SeasonState.Phase.WINTER, 0L, 100L, 0L));

        assertEquals(summer.fogDensity(), winter.fogDensity(), 0.0001);
        assertEquals(summer.skyTintRgb(), winter.skyTintRgb());
    }

    @Test
    void winter_adds_snow_particle() {
        ZoneAtmosphereCommand winter = commandFor(
            ZoneState.create("north_wastes", "North", 0.8, 4, 10L),
            new SeasonState(SeasonState.Phase.WINTER, 0L, 100L, 0L)
        );

        assertTrue(winter.particles().stream().anyMatch(p -> "snow_grain".equals(p.type())));
    }

    @Test
    void atmosphere_matrix_perf_stays_under_budget() {
        ZoneAtmosphereProfileRegistry registry = ZoneAtmosphereProfileRegistry.loadDefault();
        List<String> zones = ZoneAtmosphereProfileRegistry.REQUIRED_PROFILE_IDS.subList(0, 6);
        List<SeasonState> seasons = List.of(
            new SeasonState(SeasonState.Phase.SUMMER, 0L, 100L, 0L),
            new SeasonState(SeasonState.Phase.WINTER, 0L, 100L, 0L),
            new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 50L, 100L, 0L)
        );

        int combinations = 0;
        for (String zone : zones) {
            for (SeasonState season : seasons) {
                for (ZoneState state : List.of(
                    ZoneState.create(zone, zone, 0.8, 2, 10L),
                    ZoneState.create(zone, zone, -1.0, 5, 10L)
                )) {
                    ZoneAtmosphereCommand command = ZoneAtmospherePlanner.plan(
                        registry,
                        ZoneAtmosphereContext.of(state, season),
                        1L
                    );
                    assertNotNull(command);
                    assertTrue(command.estimatedFrameCostMs() < 2.0, zone + " exceeded atmosphere frame budget");
                    combinations++;
                }
            }
        }
        assertEquals(36, combinations);
    }

    @Test
    void zone_profiles_are_visually_distinct() {
        ZoneAtmosphereProfileRegistry registry = ZoneAtmosphereProfileRegistry.loadDefault();

        assertFalse(registry.forZone("blood_valley").fogColorRgb() == registry.forZone("spring_marsh").fogColorRgb());
        assertTrue(registry.forZone("north_wastes").fogDensity() > registry.forZone("wilderness").fogDensity());
    }

    private static ZoneAtmosphereCommand commandFor(ZoneState zoneState, SeasonState seasonState) {
        return ZoneAtmospherePlanner.plan(
            ZoneAtmosphereProfileRegistry.loadDefault(),
            ZoneAtmosphereContext.of(zoneState, seasonState),
            1L
        );
    }

    private static ZoneAtmosphereCommand commandForTsyTier(int tier) {
        return ZoneAtmospherePlanner.plan(
            ZoneAtmosphereProfileRegistry.loadDefault(),
            ZoneAtmosphereContext
                .of(ZoneState.create("tsy_lingxu", "TSY", 0.4, 5, 10L), null)
                .withTsyTier(tier),
            1L
        );
    }
}
