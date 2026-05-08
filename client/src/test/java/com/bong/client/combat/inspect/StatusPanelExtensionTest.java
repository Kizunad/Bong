package com.bong.client.combat.inspect;

import com.bong.client.combat.store.AscensionQuotaStore;
import com.bong.client.combat.store.StatusEffectStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.*;

class StatusPanelExtensionTest {
    @AfterEach void tearDown() {
        StatusEffectStore.resetForTests();
        AscensionQuotaStore.resetForTests();
    }

    @Test void groupsAreOrderedByKindEnum() {
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("buff_a", "加成", StatusEffectStore.Kind.BUFF, 1, 0, 0, "", 0),
            new StatusEffectStore.Effect("dot_a", "灼烧", StatusEffectStore.Kind.DOT, 1, 0, 0, "", 0),
            new StatusEffectStore.Effect("ctrl_a", "眩晕", StatusEffectStore.Kind.CONTROL, 1, 0, 0, "", 0)
        ));
        List<StatusPanelExtension.Group> groups = StatusPanelExtension.groupedByKind();
        assertEquals(3, groups.size());
        assertEquals(StatusEffectStore.Kind.DOT, groups.get(0).kind());
        assertEquals(StatusEffectStore.Kind.CONTROL, groups.get(1).kind());
        assertEquals(StatusEffectStore.Kind.BUFF, groups.get(2).kind());
    }

    @Test void tooltipContainsSourceAndRemaining() {
        StatusEffectStore.Effect e = new StatusEffectStore.Effect(
            "burn", "灼烧", StatusEffectStore.Kind.DOT, 3, 4_500L, 0xFFFF0000, "zombie", 2
        );
        String t = StatusPanelExtension.tooltipFor(e);
        assertTrue(t.contains("灼烧"));
        assertTrue(t.contains("×3"));
        assertTrue(t.contains("zombie"));
        assertTrue(t.contains("4.5s"));
        assertTrue(t.contains("2/5"));
    }

    @Test void ascensionQuotaLineShowsSpiritRealmQuotaSource() {
        AscensionQuotaStore.replace(new AscensionQuotaStore.State(
            1,
            2,
            1,
            100.0,
            50.0,
            "world_qi_budget.current_total"
        ));

        String line = StatusPanelExtension.ascensionQuotaLine("Spirit");
        assertEquals("当前世界化虚名额: 1 / 2 · world_qi_budget.current_total", line);
        assertTrue(StatusPanelExtension.ascensionQuotaTooltip("Spirit").contains("K: 50.0"));
    }

    @Test void ascensionQuotaLineIsHiddenBeforeSpiritRealm() {
        AscensionQuotaStore.replace(new AscensionQuotaStore.State(
            1,
            2,
            1,
            100.0,
            50.0,
            "world_qi_budget.current_total"
        ));

        assertEquals("", StatusPanelExtension.ascensionQuotaLine("Condense"));
        assertEquals("", StatusPanelExtension.ascensionQuotaTooltip("Condense"));
    }

    @Test void ascensionQuotaStoreContinuesAfterThrowingListener() {
        AtomicInteger notified = new AtomicInteger();
        AscensionQuotaStore.addListener(state -> {
            throw new IllegalStateException("boom");
        });
        AscensionQuotaStore.addListener(state -> notified.incrementAndGet());

        assertDoesNotThrow(() -> AscensionQuotaStore.replace(new AscensionQuotaStore.State(
            1,
            2,
            1,
            100.0,
            50.0,
            "world_qi_budget.current_total"
        )));
        assertEquals(1, notified.get());
    }
}
