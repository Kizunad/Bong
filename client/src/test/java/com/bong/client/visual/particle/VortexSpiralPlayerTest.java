package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;

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
