package com.bong.client.cultivation;

import com.bong.client.state.SeasonState;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BreakthroughSpectacleRendererTest {
    @Test
    void preludeDispatchesAbsorbFocusAndHeartbeat() {
        BreakthroughCinematicPayload payload = payload(BreakthroughCinematicPayload.Phase.PRELUDE, "success", false);

        BreakthroughSpectacleRenderer.SpectaclePlan plan =
            BreakthroughSpectacleRenderer.plan(payload, SeasonState.summerAt(0L), 100L);

        assertEquals("fov_zoom_in", plan.visualEffectType());
        assertEquals("breakthrough_heartbeat_slow", plan.audioRecipeId());
        assertTrue(plan.vfxEventIds().contains("bong:cultivation_absorb"));
    }

    @Test
    void chargeDispatchesAbsorbAndMeridianLoop() {
        BreakthroughCinematicPayload payload = payload(BreakthroughCinematicPayload.Phase.CHARGE, "success", false);

        BreakthroughSpectacleRenderer.SpectaclePlan plan =
            BreakthroughSpectacleRenderer.plan(payload, SeasonState.summerAt(0L), 100L);

        assertTrue(plan.vfxEventIds().contains("bong:cultivation_absorb"));
        assertTrue(plan.vfxEventIds().contains("bong:meridian_open"));
        assertEquals("breakthrough_heartbeat_fast", plan.audioRecipeId());
    }

    @Test
    void apexUsesTribulationPressureForGlobalHighRealm() {
        BreakthroughCinematicPayload payload = new BreakthroughCinematicPayload(
            "actor",
            BreakthroughCinematicPayload.Phase.APEX,
            0,
            120,
            "Solidify",
            "Spirit",
            BreakthroughCinematicPayload.Result.SUCCESS,
            false,
            0.0,
            64.0,
            0.0,
            5000.0,
            true,
            true,
            3.0,
            0.9,
            "adaptive",
            "sky_resonance",
            100L
        );

        BreakthroughSpectacleRenderer.SpectaclePlan plan =
            BreakthroughSpectacleRenderer.plan(payload, SeasonState.summerAt(0L), 100L);

        assertEquals("tribulation_pressure", plan.visualEffectType());
        assertTrue(plan.distantBillboard());
        assertEquals("breakthrough_bell", plan.audioRecipeId());
    }

    @Test
    void interruptedAftermathDispatchesFailVfxAndShake() {
        BreakthroughCinematicPayload payload = payload(BreakthroughCinematicPayload.Phase.AFTERMATH, "interrupted", true);

        BreakthroughSpectacleRenderer.SpectaclePlan plan =
            BreakthroughSpectacleRenderer.plan(payload, SeasonState.summerAt(0L), 100L);

        assertEquals("screen_shake", plan.visualEffectType());
        assertEquals("breakthrough_interrupted", plan.audioRecipeId());
        assertTrue(plan.vfxEventIds().contains("bong:breakthrough_fail"));
        assertEquals("突破被打断", plan.toastText());
    }

    @Test
    void tideTurnSeasonRaisesPressureJitterDuringCharge() {
        BreakthroughCinematicPayload payload = payload(BreakthroughCinematicPayload.Phase.CHARGE, "success", false);
        SeasonState tideTurn = new SeasonState(SeasonState.Phase.SUMMER_TO_WINTER, 10L, 100L, 0L);

        BreakthroughSpectacleRenderer.SpectaclePlan plan =
            BreakthroughSpectacleRenderer.plan(payload, tideTurn, 100L);

        assertEquals("pressure_jitter", plan.visualEffectType());
        assertTrue(plan.visualIntensity() > payload.intensity() * 0.70);
    }

    @Test
    void distantBillboardAppearsOnlyForFarWatchers() {
        BreakthroughCinematicPayload payload = new BreakthroughCinematicPayload(
            "actor",
            BreakthroughCinematicPayload.Phase.CATALYZE,
            0,
            620,
            "Solidify",
            "Spirit",
            BreakthroughCinematicPayload.Result.SUCCESS,
            false,
            1000.0,
            80.0,
            0.0,
            5000.0,
            true,
            true,
            3.0,
            0.8,
            "adaptive",
            "sky_resonance",
            100L
        );

        assertFalse(DistantBreakthroughRenderer.billboardFor(payload, 990.0, 80.0, 0.0).visible());
        DistantBreakthroughRenderer.Billboard billboard =
            DistantBreakthroughRenderer.billboardFor(payload, 0.0, 80.0, 0.0);
        assertTrue(billboard.visible());
        assertTrue(billboard.alpha() > 0.2);
        assertTrue(billboard.scale() > 0.3);
    }

    private static BreakthroughCinematicPayload payload(
        BreakthroughCinematicPayload.Phase phase,
        String result,
        boolean interrupted
    ) {
        return new BreakthroughCinematicPayload(
            "actor",
            phase,
            0,
            100,
            "Awaken",
            "Induce",
            BreakthroughCinematicPayload.Result.fromWire(result),
            interrupted,
            0.0,
            64.0,
            0.0,
            256.0,
            false,
            false,
            1.0,
            0.55,
            "adaptive",
            "fresh_spiral",
            100L
        );
    }
}
