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
        assertTrue(StatusEffectStore.presentation(0, 8).items().isEmpty());
    }

    @Test void simultaneousBatchPrioritisesDotWithoutDelayingAuthoritativeState() {
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect("buff_1", "加成", StatusEffectStore.Kind.BUFF, 1, 10_000, 0xFF00FF00, "", 0),
            new StatusEffectStore.Effect("burn", "灼烧", StatusEffectStore.Kind.DOT, 1, 2_000, 0xFFFF0000, "zombie", 3),
            new StatusEffectStore.Effect("stun", "眩晕", StatusEffectStore.Kind.CONTROL, 1, 1_000, 0xFFAAAAAA, "", 2)
        ), 0);
        var frame = StatusEffectStore.presentation(0, 8);
        assertEquals(3, StatusEffectStore.snapshot().size(), "状态立即生效，演出只展示一项");
        assertEquals("burn", frame.items().get(0).effect().id());
        assertEquals(2, frame.waiting());
    }

    @Test void overflowWaitsUntilAVisibleSlotIsReleased() {
        List<StatusEffectStore.Effect> many = new ArrayList<>();
        for (int i = 0; i < 15; i++) {
            many.add(new StatusEffectStore.Effect(
                "dot_" + i, "灼烧" + i, StatusEffectStore.Kind.DOT, 1, 60_000, 0xFFFF0000, "", 1));
        }
        StatusEffectStore.replace(many, 0);
        for (int i = 0; i <= 8; i++) StatusEffectStore.presentation(i * 1_000, 8);
        var frame = StatusEffectStore.presentation(9_000, 8);
        assertEquals(StatusEffectStore.TOP_BAR_LIMIT, frame.items().size());
        assertEquals(7, frame.waiting());
        StatusEffectStore.replace(many.subList(1, many.size()), 9_000);
        frame = StatusEffectStore.presentation(9_300, 8);
        assertEquals("dot_8", frame.items().get(7).effect().id(), "槽位释放后首个等待状态入场");
    }

    @Test void clearOnDisconnectResetsEffectsAndTimeline() {
        StatusEffectStore.replace(List.of(
            new StatusEffectStore.Effect(
                "haste", "加速", StatusEffectStore.Kind.BUFF, 1, 10_000, 0xFF00FF00, "", 0)
        ));

        StatusEffectStore.clearOnDisconnect();

        assertTrue(StatusEffectStore.snapshot().isEmpty());
        assertTrue(StatusEffectStore.presentation(System.currentTimeMillis(), 8).items().isEmpty());
    }

    @Test void kindFromWireDefaultsUnknown() {
        assertEquals(StatusEffectStore.Kind.DOT, StatusEffectStore.Kind.fromWire("dot"));
        assertEquals(StatusEffectStore.Kind.UNKNOWN, StatusEffectStore.Kind.fromWire("not-a-kind"));
        assertEquals(StatusEffectStore.Kind.UNKNOWN, StatusEffectStore.Kind.fromWire(null));
    }

    @Test void independentPillApplicationsShareAnIconUntilTheLastApplicationExpires() {
        var shortDose = new StatusEffectStore.Effect("frailty", "风烛", StatusEffectStore.Kind.DEBUFF,
            1, 1_000, 0xFFE08040, "丹药", 1);
        var longDose = new StatusEffectStore.Effect("frailty", "风烛", StatusEffectStore.Kind.DEBUFF,
            1, 10_000, 0xFFE08040, "丹药", 1);
        StatusEffectStore.replace(List.of(longDose, shortDose), 0);
        var frame = StatusEffectStore.presentation(0, 8);
        assertEquals(2, frame.items().get(0).effect().stacks(), "同 ID 的独立药效必须合并显示层数");
        assertEquals(10_000, frame.items().get(0).remainingMs(), "显示最长存续时间，不能被最后一个短药效覆盖");
        StatusEffectStore.replace(List.of(longDose), 1_000);
        frame = StatusEffectStore.presentation(1_000, 8);
        assertEquals(StatusEffectTimeline.Phase.ACTIVE, frame.items().get(0).phase(), "减少一层不重播入场");
        assertEquals(1, frame.items().get(0).effect().stacks());
    }
}
