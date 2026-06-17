package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class VortexSpiralPlayerTest {
    @Test
    void routesVortexResonanceToFieldProfileWithDefaults() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.VORTEX_RESONANCE,
            Optional.empty(),
            OptionalInt.empty(),
            OptionalInt.empty()
        ));

        assertEquals(VortexSpiralPlayer.Route.RESONANCE_FIELD, spec.route());
        assertEquals(48, spec.count(), "expected default count because resonance field omits count, actual=" + spec.count());
        assertEquals(80, spec.maxAge(), "expected default lifetime because resonance field omits duration, actual=" + spec.maxAge());
        assertEquals(0.8, spec.strength(), 0.0001);
        assertEquals(5.24, spec.radius(), 0.0001);
        assertEquals(0.752, spec.alpha(), 0.0001);
    }

    @Test
    void resonanceFieldClampsNegativeInputs() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.VORTEX_RESONANCE,
            Optional.of(-5.0),
            OptionalInt.of(-10),
            OptionalInt.of(-1)
        ));

        assertEquals(24, spec.count(), "expected count lower bound because negative count is invalid, actual=" + spec.count());
        assertEquals(30, spec.maxAge(), "expected lifetime lower bound because negative duration is invalid, actual=" + spec.maxAge());
        assertEquals(0.0, spec.strength(), 0.0001);
        assertEquals(2.2, spec.radius(), 0.0001);
        assertEquals(0.48, spec.alpha(), 0.0001);
        assertEquals(0.12, spec.ribbonWidth(), 0.0001);
    }

    @Test
    void turbulenceBurstClampsHighInputs() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.TURBULENCE_BURST,
            Optional.of(9.0),
            OptionalInt.of(500),
            OptionalInt.of(500)
        ));

        assertEquals(VortexSpiralPlayer.Route.TURBULENCE_BURST, spec.route());
        assertEquals(96, spec.count(), "expected count upper bound because burst caps emitted particles, actual=" + spec.count());
        assertEquals(80, spec.maxAge(), "expected lifetime upper bound because burst caps duration, actual=" + spec.maxAge());
        assertEquals(1.0, spec.strength(), 0.0001);
        assertEquals(1.3, spec.radius(), 0.0001);
        assertEquals(0.87, spec.alpha(), 0.0001);
        assertEquals(0.18, spec.ribbonWidth(), 0.0001);
    }

    @Test
    void defaultSpiralClampsFallbackBounds() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.EVENT_ID,
            Optional.of(-1.0),
            OptionalInt.of(0),
            OptionalInt.of(-20)
        ));

        assertEquals(VortexSpiralPlayer.Route.SPIRAL, spec.route());
        assertEquals(1, spec.count(), "expected minimum spiral count because payload count is invalid, actual=" + spec.count());
        assertEquals(1, spec.maxAge(), "expected minimum spiral lifetime because payload duration is invalid, actual=" + spec.maxAge());
        assertEquals(0.0, spec.strength(), 0.0001);
        assertEquals(0.45, spec.alpha(), 0.0001);
    }

    // ─── plan-woliu-path-v1：虚蚀路径 5 招式 effectSpec 测试 ───

    @Test
    void vortexAmbientRoutesToAmbientProfileWithDefaults() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.VORTEX_AMBIENT,
            Optional.empty(),
            OptionalInt.empty(),
            OptionalInt.empty()
        ));

        assertEquals(VortexSpiralPlayer.Route.VORTEX_AMBIENT, spec.route(),
            "expected VORTEX_AMBIENT route because event_id is vortex_ambient, actual=" + spec.route());
        assertEquals(16, spec.count(),
            "expected default ambient particle count because payload omits count, actual=" + spec.count());
        assertEquals(60, spec.maxAge(),
            "expected default ambient lifetime because payload omits duration, actual=" + spec.maxAge());
        assertEquals(0.6, spec.strength(), 0.0001);
    }

    @Test
    void vortexAmbientClampsInputBounds() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.VORTEX_AMBIENT,
            Optional.of(-1.0),
            OptionalInt.of(0),
            OptionalInt.of(0)
        ));

        assertEquals(8, spec.count(),
            "expected ambient lower count bound because payload count is zero, actual=" + spec.count());
        assertEquals(30, spec.maxAge(),
            "expected ambient lower lifetime bound because payload duration is zero, actual=" + spec.maxAge());
        assertEquals(0.0, spec.strength(), 0.0001);
        assertEquals(0.38, spec.alpha(), 0.0001);
    }

    @Test
    void voidSphereRoutesToSphereProfileWithDefaults() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.VOID_SPHERE,
            Optional.empty(),
            OptionalInt.empty(),
            OptionalInt.empty()
        ));

        assertEquals(VortexSpiralPlayer.Route.VOID_SPHERE, spec.route(),
            "expected VOID_SPHERE route because event_id is woliu_void_sphere, actual=" + spec.route());
        assertEquals(36, spec.count(),
            "expected default void sphere count because payload omits count, actual=" + spec.count());
        assertEquals(55, spec.maxAge(),
            "expected default void sphere lifetime because payload omits duration, actual=" + spec.maxAge());
        assertEquals(0.85, spec.strength(), 0.0001);
        // radius: 1.8 + 0.85 * 2.0 = 3.5
        assertEquals(3.5, spec.radius(), 0.0001);
    }

    @Test
    void voidSphereClampsHighInputs() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.VOID_SPHERE,
            Optional.of(9.0),
            OptionalInt.of(500),
            OptionalInt.of(500)
        ));

        assertEquals(72, spec.count(),
            "expected void sphere upper count bound because payload exceeds max, actual=" + spec.count());
        assertEquals(100, spec.maxAge(),
            "expected void sphere upper lifetime bound because payload exceeds max, actual=" + spec.maxAge());
        assertEquals(1.0, spec.strength(), 0.0001);
    }

    @Test
    void swallowingSpiralRoutesToSwallowingProfile() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.SWALLOWING_SPIRAL,
            Optional.empty(),
            OptionalInt.empty(),
            OptionalInt.empty()
        ));

        assertEquals(VortexSpiralPlayer.Route.SWALLOWING_SPIRAL, spec.route(),
            "expected SWALLOWING_SPIRAL route because event_id is woliu_swallowing_spiral, actual=" + spec.route());
        assertEquals(52, spec.count(),
            "expected default swallowing count because payload omits count, actual=" + spec.count());
        assertEquals(70, spec.maxAge(),
            "expected default swallowing lifetime because payload omits duration, actual=" + spec.maxAge());
        assertEquals(0.9, spec.strength(), 0.0001);
        // radius: 3.0 + 0.9 * 2.5 = 5.25
        assertEquals(5.25, spec.radius(), 0.0001);
    }

    @Test
    void swallowingSpiralClampsNegativeStrength() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.SWALLOWING_SPIRAL,
            Optional.of(-2.0),
            OptionalInt.of(24),
            OptionalInt.of(30)
        ));

        assertEquals(0.0, spec.strength(), 0.0001);
        // radius at strength=0: 3.0 + 0 * 2.5 = 3.0
        assertEquals(3.0, spec.radius(), 0.0001);
        assertEquals(24, spec.count(),
            "expected clamped count within swallowing bounds, actual=" + spec.count());
    }

    @Test
    void echoRippleRoutesToRippleProfile() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.ECHO_RIPPLE,
            Optional.empty(),
            OptionalInt.empty(),
            OptionalInt.empty()
        ));

        assertEquals(VortexSpiralPlayer.Route.ECHO_RIPPLE, spec.route(),
            "expected ECHO_RIPPLE route because event_id is woliu_echo_ripple, actual=" + spec.route());
        assertEquals(28, spec.count(),
            "expected default ripple count because payload omits count, actual=" + spec.count());
        assertEquals(38, spec.maxAge(),
            "expected default ripple lifetime because payload omits duration, actual=" + spec.maxAge());
        assertEquals(0.7, spec.strength(), 0.0001);
        // radius: 1.5 + 0.7 * 1.8 = 2.76
        assertEquals(2.76, spec.radius(), 0.0001);
    }

    @Test
    void echoRippleClampsLowCountAndDuration() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.ECHO_RIPPLE,
            Optional.of(0.5),
            OptionalInt.of(0),
            OptionalInt.of(0)
        ));

        assertEquals(12, spec.count(),
            "expected echo ripple lower count bound because payload count is zero, actual=" + spec.count());
        assertEquals(15, spec.maxAge(),
            "expected echo ripple lower lifetime bound because payload duration is zero, actual=" + spec.maxAge());
    }

    @Test
    void voidCoreCollapseRoutesToCollapseProfile() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.VOID_CORE_COLLAPSE,
            Optional.empty(),
            OptionalInt.empty(),
            OptionalInt.empty()
        ));

        assertEquals(VortexSpiralPlayer.Route.VOID_CORE_COLLAPSE, spec.route(),
            "expected VOID_CORE_COLLAPSE route because event_id is woliu_void_core_collapse, actual=" + spec.route());
        assertEquals(72, spec.count(),
            "expected default void core count because payload omits count, actual=" + spec.count());
        assertEquals(50, spec.maxAge(),
            "expected default void core lifetime because payload omits duration, actual=" + spec.maxAge());
        assertEquals(1.0, spec.strength(), 0.0001);
        // radius: 0.8 + 1.0 * 0.9 = 1.7
        assertEquals(1.7, spec.radius(), 0.0001);
        // alpha: Math.min(0.95, 0.60 + 1.0 * 0.30) = Math.min(0.95, 0.90) = 0.90
        assertEquals(0.90, spec.alpha(), 0.0001);
    }

    @Test
    void voidCoreClampsHighCountAndDuration() {
        VortexSpiralPlayer.EffectSpec spec = VortexSpiralPlayer.effectSpec(payload(
            VortexSpiralPlayer.VOID_CORE_COLLAPSE,
            Optional.of(2.0),
            OptionalInt.of(500),
            OptionalInt.of(500)
        ));

        assertEquals(96, spec.count(),
            "expected void core upper count bound because payload exceeds max, actual=" + spec.count());
        assertEquals(90, spec.maxAge(),
            "expected void core upper lifetime bound because payload exceeds max, actual=" + spec.maxAge());
        assertEquals(1.0, spec.strength(), 0.0001);
    }

    @Test
    void allErosionEventIdsHaveDistinctRoutes() {
        // Verify each erosion ID dispatches to its own unique route — no fallthrough to SPIRAL
        VortexSpiralPlayer.Route[] erosionRoutes = {
            VortexSpiralPlayer.effectSpec(payload(VortexSpiralPlayer.VORTEX_AMBIENT, Optional.empty(), OptionalInt.empty(), OptionalInt.empty())).route(),
            VortexSpiralPlayer.effectSpec(payload(VortexSpiralPlayer.VOID_SPHERE, Optional.empty(), OptionalInt.empty(), OptionalInt.empty())).route(),
            VortexSpiralPlayer.effectSpec(payload(VortexSpiralPlayer.SWALLOWING_SPIRAL, Optional.empty(), OptionalInt.empty(), OptionalInt.empty())).route(),
            VortexSpiralPlayer.effectSpec(payload(VortexSpiralPlayer.ECHO_RIPPLE, Optional.empty(), OptionalInt.empty(), OptionalInt.empty())).route(),
            VortexSpiralPlayer.effectSpec(payload(VortexSpiralPlayer.VOID_CORE_COLLAPSE, Optional.empty(), OptionalInt.empty(), OptionalInt.empty())).route(),
        };
        for (VortexSpiralPlayer.Route route : erosionRoutes) {
            assertTrue(route != VortexSpiralPlayer.Route.SPIRAL,
                "expected erosion skill to route to its own profile, not fall through to SPIRAL, actual=" + route);
        }
        // Verify all 5 are distinct from each other
        java.util.Set<VortexSpiralPlayer.Route> uniqueRoutes = new java.util.HashSet<>(java.util.Arrays.asList(erosionRoutes));
        assertEquals(5, uniqueRoutes.size(),
            "expected 5 distinct erosion routes but got " + uniqueRoutes.size() + ": " + uniqueRoutes);
    }

    @Test
    void erosionIdentifierStringsMatchServerVisualForContract() {
        // Pin test: these constants must match visual_for() particle_id values in server/src/combat/woliu_v2/skills.rs
        assertEquals("bong:vortex_ambient",          VortexSpiralPlayer.VORTEX_AMBIENT.toString(),
            "VORTEX_AMBIENT must match server visual_for(AmbientVortex).particle_id");
        assertEquals("bong:woliu_void_sphere",       VortexSpiralPlayer.VOID_SPHERE.toString(),
            "VOID_SPHERE must match server visual_for(VoidVortex).particle_id");
        assertEquals("bong:woliu_swallowing_spiral", VortexSpiralPlayer.SWALLOWING_SPIRAL.toString(),
            "SWALLOWING_SPIRAL must match server visual_for(SwallowingVortex).particle_id");
        assertEquals("bong:woliu_echo_ripple",       VortexSpiralPlayer.ECHO_RIPPLE.toString(),
            "ECHO_RIPPLE must match server visual_for(VortexEcho).particle_id");
        assertEquals("bong:woliu_void_core_collapse",VortexSpiralPlayer.VOID_CORE_COLLAPSE.toString(),
            "VOID_CORE_COLLAPSE must match server visual_for(VoidCore).particle_id");
    }

    private static VfxEventPayload.SpawnParticle payload(
        net.minecraft.util.Identifier eventId,
        Optional<Double> strength,
        OptionalInt count,
        OptionalInt durationTicks
    ) {
        return new VfxEventPayload.SpawnParticle(
            eventId,
            new double[] { 0.0, 64.0, 0.0 },
            Optional.empty(),
            OptionalInt.empty(),
            strength,
            count,
            durationTicks
        );
    }
}
