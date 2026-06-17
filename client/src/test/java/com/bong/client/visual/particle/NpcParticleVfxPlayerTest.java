package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NpcParticleVfxPlayerTest {
    private static final float EPS = 1e-4f;

    @Test
    void allNpcParticleEventIdsResolvePayloadOverrides() {
        for (NpcParticleVfxPlayer.Kind kind : NpcParticleVfxPlayer.Kind.values()) {
            NpcParticleVfxPlayer.ParticleSpec spec = NpcParticleVfxPlayer.resolveSpec(
                payload(kind.eventId(), 0x123456, 5, 7, 0.25, new double[] { 3.0, 0.0, 4.0 }),
                kind
            );

            assertEquals(kind, spec.kind(), "event " + kind.eventId() + " should keep its configured kind");
            assertEquals(channel(0x123456, 16), spec.red(), EPS);
            assertEquals(channel(0x123456, 8), spec.green(), EPS);
            assertEquals(channel(0x123456, 0), spec.blue(), EPS);
            assertEquals(5, spec.count(), "event " + kind.eventId() + " should read payload count");
            assertEquals(7, spec.maxAge(), "event " + kind.eventId() + " should read payload duration");
            assertEquals(0.25, spec.strength(), EPS);
            assertEquals(0.6, spec.direction()[0], EPS);
            assertEquals(0.0, spec.direction()[1], EPS);
            assertEquals(0.8, spec.direction()[2], EPS);
        }
    }

    @Test
    void serverPinnedDefaultsStayVisibleForMinimalPayloads() {
        assertDefault(NpcParticleVfxPlayer.Kind.SKULL_FIEND_LOCKING,
            NpcParticleVfxPlayer.ParticleShape.LINE, 0xAA0022, 18, 30);
        assertDefault(NpcParticleVfxPlayer.Kind.SKULL_FIEND_TRAIL,
            NpcParticleVfxPlayer.ParticleShape.LINE, 0x31004A, 14, 20);
        assertDefault(NpcParticleVfxPlayer.Kind.SKULL_FIEND_IMPACT,
            NpcParticleVfxPlayer.ParticleShape.LINE, 0xF2F2FF, 28, 18);
        assertDefault(NpcParticleVfxPlayer.Kind.SKULL_FIEND_STUNNED,
            NpcParticleVfxPlayer.ParticleShape.RIBBON, 0x7C2BCB, 16, 40);
        assertDefault(NpcParticleVfxPlayer.Kind.HYBRID_FORMATION,
            NpcParticleVfxPlayer.ParticleShape.RIBBON, 0xA07058, 24, 36);
        assertDefault(NpcParticleVfxPlayer.Kind.HYBRID_RAGE,
            NpcParticleVfxPlayer.ParticleShape.LINE, 0xFF4010, 8, 18);
        assertDefault(NpcParticleVfxPlayer.Kind.SUPPLY_COFFIN_EMERGE,
            NpcParticleVfxPlayer.ParticleShape.RIBBON, 0xA08050, 6, 25);
        assertDefault(NpcParticleVfxPlayer.Kind.SUPPLY_COFFIN_BREAK,
            NpcParticleVfxPlayer.ParticleShape.LINE, 0x8B6914, 12, 15);
    }

    @Test
    void bootstrapRegistersAllNpcParticleRoutes() {
        VfxBootstrap.registerDefaults();

        for (Identifier eventId : NpcParticleVfxPlayer.EVENT_IDS) {
            assertTrue(VfxRegistry.instance().contains(eventId),
                "bootstrap should register " + eventId + " so BongVfxParticleBridge no longer bridgeMisses");
            assertTrue(VfxRegistry.instance().lookup(eventId).orElseThrow() instanceof NpcParticleVfxPlayer,
                "event " + eventId + " should route to NpcParticleVfxPlayer");
        }
    }

    private static void assertDefault(
        NpcParticleVfxPlayer.Kind kind,
        NpcParticleVfxPlayer.ParticleShape shape,
        int rgb,
        int count,
        int durationTicks
    ) {
        NpcParticleVfxPlayer.ParticleSpec spec = NpcParticleVfxPlayer.resolveSpec(
            payload(kind.eventId(), null, null, null, null, null),
            kind
        );

        assertEquals(shape, spec.shape(), "event " + kind.eventId() + " should use its configured particle base");
        assertEquals(channel(rgb, 16), spec.red(), EPS);
        assertEquals(channel(rgb, 8), spec.green(), EPS);
        assertEquals(channel(rgb, 0), spec.blue(), EPS);
        assertEquals(count, spec.count(), "event " + kind.eventId() + " fallback count drifted");
        assertEquals(durationTicks, spec.maxAge(), "event " + kind.eventId() + " fallback duration drifted");
    }

    private static VfxEventPayload.SpawnParticle payload(
        Identifier eventId,
        Integer colorRgb,
        Integer count,
        Integer durationTicks,
        Double strength,
        double[] direction
    ) {
        return new VfxEventPayload.SpawnParticle(
            eventId,
            new double[] { 1.0, 64.0, -2.0 },
            direction == null ? Optional.empty() : Optional.of(direction),
            colorRgb == null ? OptionalInt.empty() : OptionalInt.of(colorRgb),
            strength == null ? Optional.empty() : Optional.of(strength),
            count == null ? OptionalInt.empty() : OptionalInt.of(count),
            durationTicks == null ? OptionalInt.empty() : OptionalInt.of(durationTicks)
        );
    }

    private static float channel(int rgb, int shift) {
        return ((rgb >> shift) & 0xFF) / 255f;
    }
}
