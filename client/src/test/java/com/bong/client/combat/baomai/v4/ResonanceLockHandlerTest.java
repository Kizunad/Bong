package com.bong.client.combat.baomai.v4;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-combat-skill-feedback-bridges-v1 P1 — ResonanceLockHandler + HudStateStore 单测。
 *
 * 测契约：
 *   - handleLock → locked=true, partnerId, startedAtTick, endsAtTick
 *   - handleLockEnd → locked=false (State.UNLOCKED)
 *   - State.progress(tick) 正确计算
 *   - State.remainingTicks(tick) 正确计算
 *   - 无效 JSON → 不崩溃
 *   - clear → State.UNLOCKED
 */
public class ResonanceLockHandlerTest {

    @BeforeEach
    void setUp() {
        ResonanceLockHudStateStore.clear();
    }

    @AfterEach
    void tearDown() {
        ResonanceLockHudStateStore.clear();
    }

    private static final String LOCK_PAYLOAD = """
            {
              "v": 1,
              "fighter_a_id": "offline:PlayerA",
              "fighter_b_id": "offline:PlayerB",
              "started_at": 1000,
              "ends_at": 1060
            }
            """;

    private static final String LOCK_END_PAYLOAD = """
            {
              "v": 1,
              "fighter_a_id": "offline:PlayerA",
              "fighter_b_id": "offline:PlayerB",
              "reason": {"type": "expired"},
              "tick": 1060
            }
            """;

    @Test
    void handleLock_setsLockedTrue() {
        ResonanceLockHandler.handleLock(LOCK_PAYLOAD);
        ResonanceLockHudStateStore.State state = ResonanceLockHudStateStore.snapshot();
        assertTrue(state.locked, "handleLock should set locked=true");
    }

    @Test
    void handleLock_setsStartedAtAndEndsAt() {
        ResonanceLockHandler.handleLock(LOCK_PAYLOAD);
        ResonanceLockHudStateStore.State state = ResonanceLockHudStateStore.snapshot();
        assertEquals(1000L, state.startedAtTick,
                "startedAtTick should match 'started_at' field in payload");
        assertEquals(1060L, state.endsAtTick,
                "endsAtTick should match 'ends_at' field in payload");
    }

    @Test
    void handleLock_progressAtStart_isZero() {
        ResonanceLockHandler.handleLock(LOCK_PAYLOAD);
        ResonanceLockHudStateStore.State state = ResonanceLockHudStateStore.snapshot();
        double progress = state.progress(1000L);
        assertEquals(0.0, progress, 1e-9,
                "progress at startedAtTick should be 0.0 (no time elapsed)");
    }

    @Test
    void handleLock_progressAtEnd_isOne() {
        ResonanceLockHandler.handleLock(LOCK_PAYLOAD);
        ResonanceLockHudStateStore.State state = ResonanceLockHudStateStore.snapshot();
        double progress = state.progress(1060L);
        assertEquals(1.0, progress, 1e-9,
                "progress at endsAtTick should be 1.0 (fully elapsed)");
    }

    @Test
    void handleLock_progressMidway() {
        ResonanceLockHandler.handleLock(LOCK_PAYLOAD);
        ResonanceLockHudStateStore.State state = ResonanceLockHudStateStore.snapshot();
        // tick 1030 = halfway through 1000-1060
        double progress = state.progress(1030L);
        assertEquals(0.5, progress, 0.01,
                "midpoint tick should yield progress ~0.5");
    }

    @Test
    void handleLock_remainingTicks() {
        ResonanceLockHandler.handleLock(LOCK_PAYLOAD);
        ResonanceLockHudStateStore.State state = ResonanceLockHudStateStore.snapshot();
        assertEquals(30L, state.remainingTicks(1030L),
                "remainingTicks at tick 1030 should be 30 (ends_at - current)");
        assertEquals(0L, state.remainingTicks(1060L),
                "remainingTicks at endsAtTick should be 0");
        assertEquals(0L, state.remainingTicks(1100L),
                "remainingTicks past endsAtTick should be 0 (not negative)");
    }

    @Test
    void handleLockEnd_setsLockedFalse() {
        ResonanceLockHandler.handleLock(LOCK_PAYLOAD);
        assertTrue(ResonanceLockHudStateStore.snapshot().locked, "should be locked before end");

        ResonanceLockHandler.handleLockEnd(LOCK_END_PAYLOAD);
        ResonanceLockHudStateStore.State state = ResonanceLockHudStateStore.snapshot();
        assertFalse(state.locked,
                "handleLockEnd should set locked=false (unlock HUD meter)");
        assertSame(ResonanceLockHudStateStore.State.UNLOCKED, state,
                "state after lock end should be the UNLOCKED sentinel");
    }

    @Test
    void handleLock_invalidJson_doesNotCrash_storeUnchanged() {
        ResonanceLockHandler.handleLock("not-json");
        assertFalse(ResonanceLockHudStateStore.snapshot().locked,
                "invalid JSON should not lock the store");
    }

    @Test
    void handleLockEnd_invalidJson_doesNotCrash() {
        ResonanceLockHandler.handleLockEnd("not-json");
        // no crash = pass; store should still be UNLOCKED
        assertFalse(ResonanceLockHudStateStore.snapshot().locked);
    }

    @Test
    void clear_resetsToUnlocked() {
        ResonanceLockHandler.handleLock(LOCK_PAYLOAD);
        assertTrue(ResonanceLockHudStateStore.snapshot().locked, "should be locked");
        ResonanceLockHudStateStore.clear();
        assertFalse(ResonanceLockHudStateStore.snapshot().locked,
                "clear() should reset to UNLOCKED");
    }

    @Test
    void unlockedState_progressAndRemaining_areSafe() {
        // UNLOCKED state should not throw on progress/remainingTicks
        ResonanceLockHudStateStore.State state = ResonanceLockHudStateStore.State.UNLOCKED;
        assertDoesNotThrow(() -> {
            double p = state.progress(1000L);
            long r = state.remainingTicks(1000L);
            // UNLOCKED: total duration = max(1, 0-0) = 1, so p should be bounded
            assertTrue(p >= 0.0 && p <= 1.0,
                    "progress on UNLOCKED state must be in [0,1], not throw");
            assertEquals(0L, r,
                    "remainingTicks on UNLOCKED state should be 0");
        });
    }
}
