package com.bong.client.practice;

import com.bong.client.combat.*;
import com.bong.client.combat.inspect.*;
import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;
import java.util.*;
import static org.junit.jupiter.api.Assertions.*;

class PracticeBindingSessionTest {
    private SkillBarConfig slots = SkillBarConfig.empty();
    private List<TechniquesListPanel.Technique> known = List.of(PracticeCatalogTest.technique("new", "dash"));
    private boolean alive = true;
    private long now;
    private final List<TechniqueIntent> sent = new ArrayList<>();
    private final PracticeBindingSession session = new PracticeBindingSession("new", () -> known, () -> slots,
        () -> alive, technique -> "", intent -> { sent.add(intent); return UiIntentResult.accepted(); }, () -> now);
    private void bind(String id) { slots = slots.withSlot(0, SkillBarEntry.skill(id, id, 0, 0, "")); }

    @Test void emptySlotWaitsForAuthoritativeStateAndTimesOutHonestly() {
        session.choose(PracticeBindingSession.Target.combat(0));
        assertEquals(new TechniqueIntent.BindChecked(false, 0, "new", ""), sent.get(0));
        assertNull(slots.slot(0));
        assertTrue(session.pending().awaiting());
        now = 6000; session.refresh();
        assertTrue(session.message().contains("未收到"));
        session.choose(PracticeBindingSession.Target.combat(0));
        bind("new"); session.refresh();
        assertNull(session.pending());
        assertEquals("绑定已同步", session.message());
    }
    @Test void occupiedSlotRequiresConfirmationAndRejectsChangedOrClosedOwner() {
        bind("old"); session.choose(PracticeBindingSession.Target.combat(0));
        assertTrue(sent.isEmpty());
        bind("another"); session.confirm();
        assertTrue(sent.isEmpty(), "不得覆盖确认期间新出现的绑定");
        session.choose(PracticeBindingSession.Target.combat(0));
        alive = false; session.confirm();
        assertTrue(sent.isEmpty(), "窗口关闭后旧回调必须失效");
        alive = true; session.choose(PracticeBindingSession.Target.combat(0)); session.confirm();
        assertEquals(new TechniqueIntent.BindChecked(false, 0, "new", "skill:another"), sent.get(0));
    }
    @Test void dashUsesDedicatedTargetAndOnlyLearnedDashCanBind() {
        session.choose(PracticeBindingSession.Target.dashKey()); session.confirm();
        assertEquals(new TechniqueIntent.BindChecked(true, -1, "new", "skill:movement.dash"), sent.get(0));
        sent.clear(); known = List.of(); session.refresh();
        session.choose(PracticeBindingSession.Target.dashKey());
        assertTrue(sent.isEmpty());
        known = List.of(PracticeCatalogTest.technique("new", "skill"));
        session.choose(PracticeBindingSession.Target.dashKey()); session.confirm();
        assertTrue(sent.isEmpty(), "攻击等普通功法不能绑定闪避键");
    }
}
