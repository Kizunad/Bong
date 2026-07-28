package com.bong.client.combat;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SkillBarKeyRouterTest {
    private final List<Integer> sent = new ArrayList<>();
    private int containerSwitches;

    @BeforeEach
    void setUp() {
        SkillBarStore.resetForTests();
        CastStateStore.resetForTests();
        containerSwitches = 0;
    }

    @AfterEach
    void tearDown() {
        SkillBarStore.resetForTests();
        CastStateStore.resetForTests();
    }

    @Test
    void disconnectClearRestoresFullNeutralStateAndPreservesListener() {
        AtomicInteger notifications = new AtomicInteger();
        SkillBarStore.addListener(config -> notifications.incrementAndGet());
        SkillBarStore.replace(SkillBarConfig.of(
            new SkillBarEntry[] { SkillBarEntry.item("earth_crumb", "土块", 0, 0, "") },
            new long[] { 2_000L }
        ));
        SkillBarStore.setSelectedSlot(0);
        assertEquals(1, notifications.get(), "前置：旧 session 写入必须通知长期 listener");

        SkillBarStore.clearOnDisconnect();

        assertSame(SkillBarConfig.empty(), SkillBarStore.snapshot(),
            "断线必须恢复完整 empty config，而非只清一个槽位");
        assertEquals(SkillBarStore.NO_SELECTED_SLOT, SkillBarStore.selectedSlot(),
            "断线必须独立清除旧 session selected slot");
        assertEquals(2, notifications.get(), "clear 必须通知且不得删除长期 listener");

        SkillBarStore.updateSlot(1, SkillBarEntry.item("fresh", "新物", 0, 0, ""));

        assertEquals("fresh", SkillBarStore.snapshot().slot(1).id());
        assertEquals(3, notifications.get(), "新 session 写入仍必须通知原 listener");
    }

    @Test
    void emptySlotPassesThroughNativeHotbar() {
        assertEquals(SkillBarKeyRouter.RouteResult.PASS_THROUGH,
            SkillBarKeyRouter.route(0, 1000L, sent::add));
        assertEquals(List.of(), sent);
    }

    @Test
    void itemSlotCancelsNativeHotbarAndTogglesSelectedSlot() {
        SkillBarStore.updateSlot(1, SkillBarEntry.item("tea", "茶", 0, 0, ""));

        assertEquals(SkillBarKeyRouter.RouteResult.ITEM_SELECTED,
            SkillBarKeyRouter.route(1, 1000L, sent::add));
        assertEquals(1, SkillBarStore.selectedSlot());
        assertTrue(SkillBarKeyRouter.shouldCancelHotbarKey(1));
        assertEquals(SkillBarStore.NO_SELECTED_SLOT, SkillBarStore.selectedSlot());
        assertEquals(List.of(), sent);
    }

    @Test
    void skillSlotSendsCastPredictsSkillSourceAndClearsSelectedItem() {
        SkillBarStore.updateSlot(0, SkillBarEntry.skill("burst_meridian.beng_quan", "崩拳", 400, 3000, ""));
        SkillBarStore.updateSlot(1, SkillBarEntry.item("earth_crumb", "土块", 0, 0, ""));
        SkillBarStore.setSelectedSlot(1);

        assertEquals(SkillBarKeyRouter.RouteResult.CAST_SENT,
            SkillBarKeyRouter.route(0, 1000L, sent::add));

        assertEquals(List.of(0), sent);
        assertEquals(CastState.Source.SKILL_BAR, CastStateStore.snapshot().source());
        assertEquals(0, CastStateStore.snapshot().slot());
        assertEquals(SkillBarStore.NO_SELECTED_SLOT, SkillBarStore.selectedSlot());
    }

    @Test
    void cooldownBlocksCast() {
        SkillBarStore.replace(SkillBarConfig.of(
            new SkillBarEntry[] { SkillBarEntry.skill("burst_meridian.beng_quan", "崩拳", 400, 3000, "") },
            new long[] { 2000L }
        ));

        assertEquals(SkillBarKeyRouter.RouteResult.COOLDOWN_BLOCKED,
            SkillBarKeyRouter.route(0, 1000L, sent::add));
        assertEquals(List.of(), sent);
    }

    @Test
    void anqiContainerSwitchOnlyRoutesWhenAnqiSkillIsConfigured() {
        assertEquals(SkillBarKeyRouter.RouteResult.PASS_THROUGH,
            SkillBarKeyRouter.routeAnqiContainerSwitch(() -> containerSwitches++));
        assertEquals(0, containerSwitches);

        SkillBarStore.updateSlot(0, SkillBarEntry.skill("anqi.multi_shot", "多发齐射", 900, 4000, ""));
        assertEquals(SkillBarKeyRouter.RouteResult.CONTAINER_SWITCH_SENT,
            SkillBarKeyRouter.routeAnqiContainerSwitch(() -> containerSwitches++));
        assertEquals(1, containerSwitches);
    }
}
