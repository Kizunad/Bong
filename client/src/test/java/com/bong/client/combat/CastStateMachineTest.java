package com.bong.client.combat;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CastStateMachineTest {

    @AfterEach
    void tearDown() {
        CastStateStore.resetForTests();
    }

    @Test
    void idleIsDefault() {
        assertTrue(CastStateStore.snapshot().isIdle());
    }

    @Test
    void beginCastTransitionsToCasting() {
        CastStateStore.beginCast(3, 1200, 1000L);
        CastState s = CastStateStore.snapshot();
        assertTrue(s.isCasting());
        assertEquals(3, s.slot());
        assertEquals(1200, s.durationMs());
    }

    @Test
    void secondBeginIsIgnoredWhileCasting() {
        CastStateStore.beginCast(3, 1000, 0L);
        CastStateStore.beginCast(4, 2000, 500L);
        CastState s = CastStateStore.snapshot();
        assertEquals(3, s.slot(), "second beginCast during Casting must be ignored");
    }

    @Test
    void tickAutoCompletesAfterDuration() {
        CastStateStore.beginCast(0, 500, 0L);
        CastStateStore.tick(400L);
        assertTrue(CastStateStore.snapshot().isCasting(), "not yet complete at 400ms of 500ms");
        CastStateStore.tick(600L);
        assertEquals(CastState.Phase.COMPLETE, CastStateStore.snapshot().phase());
    }

    @Test
    void interruptMovementDoesNotConsumeItem() {
        CastStateStore.beginCast(2, 2000, 0L);
        CastStateStore.interrupt(CastOutcome.INTERRUPT_MOVEMENT, 300L);
        CastState s = CastStateStore.snapshot();
        assertEquals(CastState.Phase.INTERRUPT, s.phase());
        assertEquals(CastOutcome.INTERRUPT_MOVEMENT, s.outcome());
        assertFalse(s.outcome().consumesItem(), "movement interrupt must not consume item");
    }

    @Test
    void completedConsumesItem() {
        CastStateStore.beginCast(1, 200, 0L);
        CastStateStore.tick(300L);
        assertTrue(CastStateStore.snapshot().outcome().consumesItem());
    }

    @Test
    void terminalPhasesRevertToIdleAfterFadeOut() {
        CastStateStore.beginCast(0, 100, 0L);
        CastStateStore.tick(150L);
        assertEquals(CastState.Phase.COMPLETE, CastStateStore.snapshot().phase());
        CastStateStore.tick(160L);
        assertEquals(CastState.Phase.COMPLETE, CastStateStore.snapshot().phase());
        CastStateStore.tick(150L + 400L); // past 300ms fade
        assertEquals(CastState.Phase.IDLE, CastStateStore.snapshot().phase());
    }

    @Test
    void progressIsClamped() {
        CastState casting = CastState.casting(0, 1000, 0L);
        assertEquals(0.0f, casting.progress(-100L), 1e-6);
        assertEquals(0.5f, casting.progress(500L), 1e-6);
        assertEquals(1.0f, casting.progress(1500L), 1e-6);
    }

    @Test
    void movementInterruptRulePredicate() {
        assertFalse(CastInterruptRules.movementInterrupts(0.2));
        assertTrue(CastInterruptRules.movementInterrupts(0.31));
    }

    @Test
    void contamInterruptUsesRateBudget() {
        // maxHp=100, duration=1000ms → threshold = 1s * 0.05 * 100 = 5 HP
        assertFalse(CastInterruptRules.contamInterrupts(4.9, 100, 1000));
        assertTrue(CastInterruptRules.contamInterrupts(5.1, 100, 1000));
    }

    @Test
    void controlInterruptOnlyForHardCCs() {
        assertTrue(CastInterruptRules.controlInterrupts(CastInterruptRules.ControlEffect.STUN));
        assertTrue(CastInterruptRules.controlInterrupts(CastInterruptRules.ControlEffect.SILENCED_PHYSICAL));
        assertTrue(CastInterruptRules.controlInterrupts(CastInterruptRules.ControlEffect.KNOCKBACK));
        assertTrue(CastInterruptRules.controlInterrupts(CastInterruptRules.ControlEffect.CHARMED));
        assertFalse(CastInterruptRules.controlInterrupts(CastInterruptRules.ControlEffect.SLOWED));
        assertFalse(CastInterruptRules.controlInterrupts(CastInterruptRules.ControlEffect.DAMAGE_AMP));
    }
}
