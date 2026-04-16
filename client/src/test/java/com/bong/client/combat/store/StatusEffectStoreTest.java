package com.bong.client.combat.store;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class StatusEffectStoreTest {
    @AfterEach void tearDown() { StatusEffectStore.resetForTests(); }

    @Test void emptyByDefault() {
        assertTrue(StatusEffectStore.snapshot().isEmpty());
        assertTrue(StatusEffectStore.topBar().isEmpty());
    }

    @Test void topBarPrioritisesDotOverBuff() {
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("buff_1", "加成", StatusEffectStore.Kind.BUFF, 1, 10_000, 0xFF00FF00, "", 0),
            new StatusEffectStore.Effect("burn", "灼烧", StatusEffectStore.Kind.DOT, 1, 2_000, 0xFFFF0000, "zombie", 3),
            new StatusEffectStore.Effect("stun", "眩晕", StatusEffectStore.Kind.CONTROL, 1, 1_000, 0xFFAAAAAA, "", 2)
        ));
        List<StatusEffectStore.Effect> top = StatusEffectStore.topBar();
        assertEquals("burn", top.get(0).id());
        assertEquals("stun", top.get(1).id());
        assertEquals("buff_1", top.get(2).id());
    }

    @Test void topBarLimitsTo8() {
        List<StatusEffectStore.Effect> many = new ArrayList<>();
        for (int i = 0; i < 15; i++) {
            many.add(new StatusEffectStore.Effect(
                "dot_" + i, "灼烧" + i, StatusEffectStore.Kind.DOT, 1, 1_000 + i, 0xFFFF0000, "", 1));
        }
        StatusEffectStore.replace(many);
        assertEquals(StatusEffectStore.TOP_BAR_LIMIT, StatusEffectStore.topBar().size());
    }

    @Test void kindFromWireDefaultsUnknown() {
        assertEquals(StatusEffectStore.Kind.DOT, StatusEffectStore.Kind.fromWire("dot"));
        assertEquals(StatusEffectStore.Kind.UNKNOWN, StatusEffectStore.Kind.fromWire("not-a-kind"));
        assertEquals(StatusEffectStore.Kind.UNKNOWN, StatusEffectStore.Kind.fromWire(null));
    }
}
