package com.bong.client.omen;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class OmenStateStoreTest {
    @AfterEach
    void reset() {
        OmenStateStore.resetForTests();
    }

    @Test
    void kindFromEventIdMapsOnlyKnownOmenEvents() {
        assertNull(OmenStateStore.kindFromEventId(null),
            "null event id should not map to an omen kind");
        assertNull(OmenStateStore.kindFromEventId(new Identifier("bong", "world_omen_unknown")),
            "unknown omen event id should not map to an omen kind");
        assertEquals(OmenStateStore.Kind.PSEUDO_VEIN,
            OmenStateStore.kindFromEventId(new Identifier("bong", "world_omen_pseudo_vein")),
            "pseudo vein omen id should map to PSEUDO_VEIN");
        assertEquals(OmenStateStore.Kind.BEAST_TIDE,
            OmenStateStore.kindFromEventId(new Identifier("bong", "world_omen_beast_tide")),
            "beast tide omen id should map to BEAST_TIDE");
        assertEquals(OmenStateStore.Kind.REALM_COLLAPSE,
            OmenStateStore.kindFromEventId(new Identifier("bong", "world_omen_realm_collapse")),
            "realm collapse omen id should map to REALM_COLLAPSE");
        assertEquals(OmenStateStore.Kind.KARMA_BACKLASH,
            OmenStateStore.kindFromEventId(new Identifier("bong", "world_omen_karma_backlash")),
            "karma backlash omen id should map to KARMA_BACKLASH");
    }

    @Test
    void noteClampsDurationAndStrengthBounds() {
        OmenStateStore.note(payload("world_omen_pseudo_vein", -0.25, 1), 1_000L);
        OmenStateStore.Entry shortEntry = OmenStateStore.snapshot(1_000L).entries().get(0);

        assertEquals(2_000L, shortEntry.expiresAtMillis(),
            "duration below visible floor should clamp to 1000ms from now");
        assertEquals(0.0, shortEntry.strength(),
            "negative strength should clamp to the lower bound");

        OmenStateStore.note(payload("world_omen_beast_tide", 1.5, 1_000), 2_000L);
        OmenStateStore.Entry longEntry = OmenStateStore.snapshot(2_000L)
            .entries()
            .stream()
            .filter(entry -> entry.kind() == OmenStateStore.Kind.BEAST_TIDE)
            .findFirst()
            .orElseThrow();

        assertEquals(17_000L, longEntry.expiresAtMillis(),
            "duration above visible ceiling should clamp to 15000ms from now");
        assertEquals(1.0, longEntry.strength(),
            "strength above one should clamp to the upper bound");
    }

    @Test
    void entryRejectsInvalidShapeAndDefensivelyCopiesOrigin() {
        assertThrows(NullPointerException.class,
            () -> new OmenStateStore.Entry(null, new double[] { 0.0, 64.0, 0.0 }, 0.5, 2_000L),
            "Entry should reject null kind");
        assertThrows(IllegalArgumentException.class,
            () -> new OmenStateStore.Entry(
                OmenStateStore.Kind.PSEUDO_VEIN,
                new double[] { 0.0, 64.0 },
                0.5,
                2_000L
            ),
            "Entry should reject origin arrays that are not xyz triples");

        assertEquals(0.0,
            new OmenStateStore.Entry(
                OmenStateStore.Kind.PSEUDO_VEIN,
                new double[] { 0.0, 64.0, 0.0 },
                Double.NaN,
                2_000L
            ).strength(),
            "NaN strength should clamp to zero");
        assertEquals(0.0,
            new OmenStateStore.Entry(
                OmenStateStore.Kind.PSEUDO_VEIN,
                new double[] { 0.0, 64.0, 0.0 },
                Double.POSITIVE_INFINITY,
                2_000L
            ).strength(),
            "infinite strength should clamp to zero");

        double[] origin = new double[] { 1.0, 64.0, 2.0 };
        OmenStateStore.Entry entry = new OmenStateStore.Entry(
            OmenStateStore.Kind.BEAST_TIDE,
            origin,
            0.5,
            2_000L
        );
        origin[0] = 99.0;
        assertArrayEquals(new double[] { 1.0, 64.0, 2.0 }, entry.origin(),
            "Entry should copy origin on construction");

        double[] exposed = entry.origin();
        exposed[1] = 99.0;
        assertArrayEquals(new double[] { 1.0, 64.0, 2.0 }, entry.origin(),
            "Entry origin accessor should return a defensive copy");
    }

    @Test
    void snapshotKeepsEntryAtExpiryBoundaryAndRemovesAfterward() {
        OmenStateStore.note(payload("world_omen_pseudo_vein", 0.5, 20), 1_000L);

        assertTrue(!OmenStateStore.snapshot(2_000L).entries().isEmpty(),
            "snapshot at expiresAtMillis should keep the omen active");
        assertTrue(OmenStateStore.snapshot(2_001L).entries().isEmpty(),
            "snapshot after expiresAtMillis should remove the omen");
    }

    @Test
    void noteUpsertsByKindAndOriginButKeepsParallelZones() {
        OmenStateStore.note(payload("world_omen_beast_tide", 0.2, 20), 1_000L);
        OmenStateStore.note(payload(
            "world_omen_beast_tide",
            0.4,
            20,
            new double[] { 120.0, 64.0, 0.0 }
        ), 1_050L);
        OmenStateStore.note(payload("world_omen_beast_tide", 0.9, 20), 1_100L);

        OmenStateStore.Snapshot snapshot = OmenStateStore.snapshot(1_100L);
        assertEquals(2, snapshot.entries().size(),
            "same kind from different origins should keep parallel omen entries");
        OmenStateStore.Entry refreshed = snapshot.entries()
            .stream()
            .filter(entry -> entry.origin()[0] == 0.0)
            .findFirst()
            .orElseThrow();
        assertEquals(0.9, refreshed.strength(),
            "same kind and same origin should replace the old entry");
        assertEquals(2_100L, refreshed.expiresAtMillis(),
            "same kind and same origin should expose the latest expiry");
    }

    private static VfxEventPayload.SpawnParticle payload(String path, double strength, int durationTicks) {
        return payload(path, strength, durationTicks, new double[] { 0.0, 64.0, 0.0 });
    }

    private static VfxEventPayload.SpawnParticle payload(
        String path,
        double strength,
        int durationTicks,
        double[] origin
    ) {
        return new VfxEventPayload.SpawnParticle(
            new Identifier("bong", path),
            origin,
            Optional.empty(),
            OptionalInt.empty(),
            Optional.of(strength),
            OptionalInt.of(12),
            OptionalInt.of(durationTicks)
        );
    }
}
