package com.bong.client.combat.inspect;

import com.bong.client.combat.store.StatusEffectStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class StatusPanelExtensionTest {
    @AfterEach void tearDown() { StatusEffectStore.resetForTests(); }

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
}
