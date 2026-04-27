package com.bong.client.combat;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class SkillBarKeyRouterTest {
    private final List<Integer> sent = new ArrayList<>();

    @BeforeEach
    void setUp() {
        SkillBarStore.resetForTests();
        CastStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        SkillBarStore.resetForTests();
        CastStateStore.resetForTests();
    }

    @Test
    void emptyAndItemSlotsPassThroughNativeHotbar() {
        assertEquals(SkillBarKeyRouter.RouteResult.PASS_THROUGH,
            SkillBarKeyRouter.route(0, 1000L, sent::add));
        SkillBarStore.updateSlot(1, SkillBarEntry.item("tea", "茶", 0, 0, ""));
        assertEquals(SkillBarKeyRouter.RouteResult.PASS_THROUGH,
            SkillBarKeyRouter.route(1, 1000L, sent::add));
        assertEquals(List.of(), sent);
    }

    @Test
    void skillSlotSendsCastAndPredictsSkillSource() {
        SkillBarStore.updateSlot(0, SkillBarEntry.skill("burst_meridian.beng_quan", "崩拳", 400, 3000, ""));

        assertEquals(SkillBarKeyRouter.RouteResult.CAST_SENT,
            SkillBarKeyRouter.route(0, 1000L, sent::add));

        assertEquals(List.of(0), sent);
        assertEquals(CastState.Source.SKILL_BAR, CastStateStore.snapshot().source());
        assertEquals(0, CastStateStore.snapshot().slot());
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
}
