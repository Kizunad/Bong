package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;

class FurnitureAuraVfxPlayerTest {
    private static final float EPS = 1e-4f;

    private static VfxEventPayload.SpawnParticle payload(
        OptionalInt colorRgb,
        Optional<Double> strength,
        OptionalInt count,
        OptionalInt durationTicks
    ) {
        return new VfxEventPayload.SpawnParticle(
            BedRestAuraPlayer.EVENT_ID,
            new double[] { 1.0, 64.0, -2.0 },
            Optional.empty(),
            colorRgb,
            strength,
            count,
            durationTicks
        );
    }

    private static float channel(int rgb, int shift) {
        return ((rgb >> shift) & 0xFF) / 255f;
    }

    @Test
    void bedRestDefaultsAndFallbackColorArePinned() {
        FurnitureAuraParticleSpec spec = BedRestAuraPlayer.resolveSpec(
            payload(OptionalInt.empty(), Optional.empty(), OptionalInt.empty(), OptionalInt.empty())
        );

        assertEquals(channel(0xE8C97A, 16), spec.red(), EPS);
        assertEquals(channel(0xE8C97A, 8), spec.green(), EPS);
        assertEquals(channel(0xE8C97A, 0), spec.blue(), EPS);
        assertEquals(0.75f, spec.alpha(), EPS);
        assertEquals(6, spec.count());
        assertEquals(12, spec.maxAge());
    }

    @Test
    void bedRestClampsStrengthCountAndDurationBoundaries() {
        assertEquals(0.45f, BedRestAuraPlayer.resolveSpec(
            payload(OptionalInt.of(0x010203), Optional.of(0.1), OptionalInt.of(0), OptionalInt.of(0))
        ).alpha(), EPS);
        assertEquals(0.45f, BedRestAuraPlayer.resolveSpec(
            payload(OptionalInt.of(0x010203), Optional.of(0.45), OptionalInt.of(1), OptionalInt.of(1))
        ).alpha(), EPS);
        FurnitureAuraParticleSpec maxSpec = BedRestAuraPlayer.resolveSpec(
            payload(OptionalInt.of(0x010203), Optional.of(2.0), OptionalInt.of(17), OptionalInt.of(41))
        );

        assertEquals(channel(0x010203, 16), maxSpec.red(), EPS);
        assertEquals(channel(0x010203, 8), maxSpec.green(), EPS);
        assertEquals(channel(0x010203, 0), maxSpec.blue(), EPS);
        assertEquals(0.9f, maxSpec.alpha(), EPS);
        assertEquals(16, maxSpec.count());
        assertEquals(40, maxSpec.maxAge());
        assertEquals(0.9f, BedRestAuraPlayer.resolveSpec(
            payload(OptionalInt.of(0x010203), Optional.of(0.9), OptionalInt.of(16), OptionalInt.of(40))
        ).alpha(), EPS);
    }

    @Test
    void meditationDefaultsAndFallbackColorArePinned() {
        FurnitureAuraParticleSpec spec = MeditationAuraPlayer.resolveSpec(
            payload(OptionalInt.empty(), Optional.empty(), OptionalInt.empty(), OptionalInt.empty())
        );

        assertEquals(channel(0xBFD8C8, 16), spec.red(), EPS);
        assertEquals(channel(0xBFD8C8, 8), spec.green(), EPS);
        assertEquals(channel(0xBFD8C8, 0), spec.blue(), EPS);
        assertEquals(0.75f, spec.alpha(), EPS);
        assertEquals(8, spec.count());
        assertEquals(16, spec.maxAge());
    }

    @Test
    void meditationClampsStrengthCountAndDurationBoundaries() {
        assertEquals(0.4f, MeditationAuraPlayer.resolveSpec(
            payload(OptionalInt.of(0xA0B0C0), Optional.of(0.1), OptionalInt.of(0), OptionalInt.of(0))
        ).alpha(), EPS);
        assertEquals(0.4f, MeditationAuraPlayer.resolveSpec(
            payload(OptionalInt.of(0xA0B0C0), Optional.of(0.4), OptionalInt.of(1), OptionalInt.of(1))
        ).alpha(), EPS);
        FurnitureAuraParticleSpec maxSpec = MeditationAuraPlayer.resolveSpec(
            payload(OptionalInt.of(0xA0B0C0), Optional.of(2.0), OptionalInt.of(25), OptionalInt.of(49))
        );

        assertEquals(channel(0xA0B0C0, 16), maxSpec.red(), EPS);
        assertEquals(channel(0xA0B0C0, 8), maxSpec.green(), EPS);
        assertEquals(channel(0xA0B0C0, 0), maxSpec.blue(), EPS);
        assertEquals(0.85f, maxSpec.alpha(), EPS);
        assertEquals(24, maxSpec.count());
        assertEquals(48, maxSpec.maxAge());
        assertEquals(0.85f, MeditationAuraPlayer.resolveSpec(
            payload(OptionalInt.of(0xA0B0C0), Optional.of(0.85), OptionalInt.of(24), OptionalInt.of(48))
        ).alpha(), EPS);
    }

    @Test
    void playersReturnWithoutMinecraftClient() {
        VfxEventPayload.SpawnParticle emptyPayload =
            payload(OptionalInt.empty(), Optional.empty(), OptionalInt.empty(), OptionalInt.empty());

        assertDoesNotThrow(() -> new BedRestAuraPlayer().play(null, emptyPayload));
        assertDoesNotThrow(() -> new MeditationAuraPlayer().play(null, emptyPayload));
    }
}
