package com.bong.client.combat.store;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class DamageFloaterStoreTest {
    @AfterEach void tearDown() { DamageFloaterStore.resetForTests(); }

    @Test void emptyByDefault() {
        assertTrue(DamageFloaterStore.snapshot(0L).isEmpty());
    }

    @Test void publishedFloaterVisibleWithinLifetime() {
        DamageFloaterStore.publish(new DamageFloaterStore.Floater(
            0, 0, 0, "12", 0xFFFF0000, DamageFloaterStore.Kind.HIT, 1_000L
        ));
        assertEquals(1, DamageFloaterStore.snapshot(1_500L).size());
    }

    @Test void expiresAfterLifetime() {
        DamageFloaterStore.publish(new DamageFloaterStore.Floater(
            0, 0, 0, "12", 0xFFFF0000, DamageFloaterStore.Kind.HIT, 1_000L
        ));
        long later = 1_000L + DamageFloaterStore.LIFETIME_MS + 100L;
        DamageFloaterStore.expire(later);
        assertTrue(DamageFloaterStore.snapshot(later).isEmpty());
    }

    @Test void boundedByMaxEntries() {
        for (int i = 0; i < DamageFloaterStore.MAX_ENTRIES + 10; i++) {
            DamageFloaterStore.publish(new DamageFloaterStore.Floater(
                0, 0, 0, Integer.toString(i), 0, DamageFloaterStore.Kind.HIT, 1_000L
            ));
        }
        assertEquals(DamageFloaterStore.MAX_ENTRIES, DamageFloaterStore.snapshot(1_500L).size());
    }
}
