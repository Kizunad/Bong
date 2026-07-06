package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import com.bong.client.omen.OmenStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;

class OmenParticlePlayerTest {
    @AfterEach
    void reset() {
        OmenStateStore.resetForTests();
    }

    @Test
    void playRecordsTideSkyOmenInHudStore() {
        VfxEventPayload.SpawnParticle payload = new VfxEventPayload.SpawnParticle(
            OmenParticlePlayer.TIDE_SKY,
            new double[] { 0.0, 64.0, 0.0 },
            Optional.empty(),
            OptionalInt.empty(),
            Optional.of(0.7),
            OptionalInt.of(12),
            OptionalInt.of(200)
        );

        new OmenParticlePlayer().play(null, payload);

        OmenStateStore.Entry entry = OmenStateStore.snapshot(System.currentTimeMillis())
            .entries()
            .get(0);
        assertEquals(OmenStateStore.Kind.TIDE_SKY, entry.kind(),
            "tide sky omen VFX player should feed OmenStateStore even in headless tests");
    }
}
