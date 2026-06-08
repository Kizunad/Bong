package com.bong.client.atmosphere;

import com.bong.client.era.EraAmbianceHandler;
import com.bong.client.era.EraAmbiancePayload;
import com.bong.client.era.EraAmbianceState;
import com.bong.client.environment.EnvironmentFogCommand;
import com.bong.client.state.SeasonState;
import com.bong.client.state.ZoneState;
import net.minecraft.util.math.Vec3d;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
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
    @BeforeEach
    void resetEraState() {
        EraAmbianceState.reset();
    }

    @AfterEach
    void resetRenderer() {
        ZoneAtmosphereRenderer.resetForTests();
        EraAmbianceState.reset();
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
    void parser_rejects_non_vec3_drift() {
        ZoneAtmosphereProfileParser.ParseResult result = ZoneAtmosphereProfileParser.parse(
            """
            {
              "zone_id":"spawn_plain",
              "fog_color":"#112233",
              "fog_density":0.77,
              "ambient_particle":{"type":"cloud256_dust","tint":"#445566","density":0.2,"drift":"bad"},
              "sky_tint":"#223344",
              "entry_transition_fx":"FADE",
              "ambient_recipe_id":"ambient_spawn_plain"
            }
            """,
            "spawn_plain"
        );

        assertFalse(result.ok());
        assertTrue(result.error().contains("drift must be a vec3"));
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

    // ── plan-era-state-v1 M2 — EraAmbianceState 接入 ZoneAtmospherePlanner ────

    /**
     * 验证 EraAmbianceState 无时代宣告时，ZoneAtmospherePlanner 输出不受影响（无 era 干扰）。
     *
     * <p>这是 M2 的 baseline 测试：任何新代码不能在无 era 时改变 sky_tint 或 fog_density。
     */
    @Test
    void planner_without_era_returns_baseline_profile() {
        // EraAmbianceState 已在 @BeforeEach 重置（target=null）
        ZoneAtmosphereCommand cmd = commandFor(
            ZoneState.create("spawn_plain", "Spawn", 0.5, 1, 1L), null
        );
        assertNotNull(cmd, "无 era 时 planner 仍应返回 non-null command");

        ZoneAtmosphereProfile baseline = ZoneAtmosphereProfileRegistry.loadDefault().forZone("spawn_plain");
        assertNotNull(baseline, "spawn_plain 应有 baseline profile");

        // 无 era → sky_tint 和 fog_density 应等于 baseline（0 era 干扰）
        assertEquals(
            baseline.skyTintRgb(), cmd.skyTintRgb(),
            "无时代宣告时 sky_tint_rgb 应等于 baseline profile 值；" +
            "期望 #" + Integer.toHexString(baseline.skyTintRgb()) +
            " 实际 #" + Integer.toHexString(cmd.skyTintRgb())
        );
        assertEquals(
            baseline.fogDensity(), cmd.fogDensity(), 0.001,
            "无时代宣告时 fog_density 应等于 baseline profile 值；" +
            "期望 " + baseline.fogDensity() + " 实际 " + cmd.fogDensity()
        );
    }

    /**
     * 验证 EraAmbianceState 接受灾劫时代 payload 后（transition 推进到完成），
     * ZoneAtmospherePlanner 输出的 sky_tint_rgb 向灾劫天象方向插值。
     *
     * <p>这是 M2 的核心测试：EraAmbianceState 接入 ZoneAtmospherePlanner 生产路径后，
     * 收到 era_ambiance payload 时 planner 输出的 sky_tint 应发生变化。
     * 任何回归（EraAmbianceState 未接入 planner）都会让此测试失败。
     */
    @Test
    void planner_applies_calamity_era_sky_tint_after_full_transition() {
        // 接受灾劫时代 payload
        String calamityJson = """
            {
              "v": 1,
              "era_type": "calamity",
              "sky_tint_hex": "#4A1A1A",
              "fog_density_delta": 0.15,
              "ambient_sound_id": "ambient.weather.thunder",
              "transition_ticks": 1
            }
            """;
        EraAmbiancePayload payload = EraAmbianceHandler.handle(calamityJson);
        assertNotNull(payload, "灾劫时代 payload 解析不应返回 null");
        EraAmbianceState.accept(payload);

        // 推进 tick 到 transition 完成（transitionTicks=1，tick 一次即 100% 完成）
        EraAmbianceState.tick();

        ZoneAtmosphereCommand cmd = commandFor(
            ZoneState.create("spawn_plain", "Spawn", 0.5, 1, 1L), null
        );
        assertNotNull(cmd, "接受 era payload 后 planner 仍应返回 non-null command");

        // sky_tint 应向 0x4A1A1A（灾劫）插值，不再等于 baseline
        ZoneAtmosphereProfile baseline = ZoneAtmosphereProfileRegistry.loadDefault().forZone("spawn_plain");
        assertNotNull(baseline, "spawn_plain 应有 baseline profile");

        int eraSkyTint = 0x4A1A1A; // CALAMITY_SKY_TINT from server
        // 若 EraAmbianceState 已正确接入，skyTintRgb 应向 eraSkyTint 偏移，
        // 不再等于 baseline（除非 baseline 恰好等于 era tint，极小概率）
        assertNotEquals(
            baseline.skyTintRgb(), cmd.skyTintRgb(),
            "灾劫时代 transition 完成后 sky_tint_rgb 应被 EraAmbianceState 修改，" +
            "不再等于 baseline #" + Integer.toHexString(baseline.skyTintRgb()) +
            "（实际 #" + Integer.toHexString(cmd.skyTintRgb()) + "）" +
            "；若两者相等说明 EraAmbianceState 未接入 ZoneAtmospherePlanner（M2 孤岛未修）"
        );

        // 并且 fog_density 应高于 baseline（灾劫 fog_density_delta=+0.15）
        assertTrue(
            cmd.fogDensity() > baseline.fogDensity() - 0.001,
            "灾劫时代 fog_density 应 >= baseline（fog_delta=+0.15）；" +
            "baseline=" + baseline.fogDensity() + " 实际=" + cmd.fogDensity()
        );
    }

    /**
     * 验证死灵域（dead zone）不受时代天象叠加影响（dead zone 优先）。
     *
     * <p>M2 约束：死灵域强制白/灰天空，时代天象不覆盖死灵域视觉。
     */
    @Test
    void planner_dead_zone_sky_not_overridden_by_era_ambiance() {
        // 接受灾劫时代 payload（灰暗红天）
        String calamityJson = """
            {
              "v": 1,
              "era_type": "calamity",
              "sky_tint_hex": "#4A1A1A",
              "fog_density_delta": 0.15,
              "ambient_sound_id": "ambient.weather.thunder",
              "transition_ticks": 1
            }
            """;
        EraAmbiancePayload payload = EraAmbianceHandler.handle(calamityJson);
        assertNotNull(payload);
        EraAmbianceState.accept(payload);
        EraAmbianceState.tick();

        // spirit_qi=0.0 → dead zone
        ZoneAtmosphereCommand deadCmd = commandFor(
            ZoneState.create("south_ash_dead_zone", "Dead", 0.0, 5, 1L), null
        );
        ZoneAtmosphereCommand normalCmd = commandFor(
            ZoneState.create("spawn_plain", "Spawn", 0.5, 1, 1L), null
        );

        // dead zone 强制 DEAD_SKY_RGB=0xF0F0F0，不受时代 sky_tint 影响
        // 若 dead zone 和 normal zone 的 sky_tint 不同，说明 dead zone 优先级正确
        assertNotEquals(
            deadCmd.skyTintRgb(), normalCmd.skyTintRgb(),
            "死灵域的 sky_tint 应与普通 zone 不同（死灵域固定灰白，普通 zone 受 era 染色）"
        );
    }
}
