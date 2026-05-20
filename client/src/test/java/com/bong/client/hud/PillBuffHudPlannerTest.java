package com.bong.client.hud;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PillBuffHudPlannerTest {

    @AfterEach
    void cleanup() {
        PillBuffHudPlanner.clear();
    }

    @Test
    void pillBuffHudShowsActiveBuff() {
        PillBuffHudPlanner.updateBuff("huo_xue_dan", 3000, 1.0);
        List<PillBuffHudPlanner.PillBuff> buffs = PillBuffHudPlanner.activeBuffs();
        assertEquals(1, buffs.size(), "should have exactly 1 active buff after updateBuff");
        assertEquals("huo_xue_dan", buffs.get(0).buffId());
        assertEquals(3000, buffs.get(0).remainingTicks());
        assertEquals(1.0, buffs.get(0).effectMultiplier(), 1e-9);
        assertFalse(buffs.get(0).isExpired(), "buff with 3000 ticks should not be expired");
        assertTrue(buffs.get(0).progress() > 0.0f, "progress should be > 0 for active buff");
    }

    @Test
    void pillBuffHudHidesOnExpiry() {
        PillBuffHudPlanner.updateBuff("tie_bi_san", 0, 1.2);
        List<PillBuffHudPlanner.PillBuff> buffs = PillBuffHudPlanner.activeBuffs();
        assertTrue(buffs.isEmpty(), "buff with 0 remaining ticks should not appear in activeBuffs");
    }

    @Test
    void pillBuffHudStacksTwoBuffs() {
        PillBuffHudPlanner.updateBuff("huo_xue_dan", 3000, 1.0);
        PillBuffHudPlanner.updateBuff("tie_bi_san", 1500, 1.2);
        List<PillBuffHudPlanner.PillBuff> buffs = PillBuffHudPlanner.activeBuffs();
        assertEquals(2, buffs.size(), "should have 2 active buffs");
        assertTrue(
            buffs.stream().anyMatch(b -> "huo_xue_dan".equals(b.buffId())),
            "huo_xue_dan should be in active buffs"
        );
        assertTrue(
            buffs.stream().anyMatch(b -> "tie_bi_san".equals(b.buffId())),
            "tie_bi_san should be in active buffs"
        );
    }

    @Test
    void updateBuffReplacesSameBuffId() {
        PillBuffHudPlanner.updateBuff("huo_xue_dan", 3000, 1.0);
        PillBuffHudPlanner.updateBuff("huo_xue_dan", 1500, 0.8);
        List<PillBuffHudPlanner.PillBuff> buffs = PillBuffHudPlanner.activeBuffs();
        assertEquals(1, buffs.size(), "duplicate buff_id should replace, not stack");
        assertEquals(1500, buffs.get(0).remainingTicks(), "should have updated remaining_ticks");
        assertEquals(0.8, buffs.get(0).effectMultiplier(), 1e-9, "should have updated multiplier");
    }

    @Test
    void clearRemovesAllBuffs() {
        PillBuffHudPlanner.updateBuff("huo_xue_dan", 3000, 1.0);
        PillBuffHudPlanner.updateBuff("tie_bi_san", 1500, 1.2);
        PillBuffHudPlanner.clear();
        assertTrue(PillBuffHudPlanner.activeBuffs().isEmpty(), "clear should remove all buffs");
    }

    @Test
    void expiredBuffIsRemovedFromActiveBuffs() {
        PillBuffHudPlanner.PillBuff expired = new PillBuffHudPlanner.PillBuff("old", 0, 1.0);
        assertTrue(expired.isExpired(), "buff with 0 ticks should report expired");
        assertEquals(0.0f, expired.progress(), 1e-6f, "expired buff progress should be 0");
    }

    @Test
    void progressClampedToOneForLargeRemainingTicks() {
        PillBuffHudPlanner.PillBuff big = new PillBuffHudPlanner.PillBuff("mega", 99999, 2.0);
        assertEquals(1.0f, big.progress(), 1e-6f, "progress should be clamped to 1.0 for large ticks");
    }

    @Test
    void negativeTicksTreatedAsExpired() {
        PillBuffHudPlanner.updateBuff("negative", -1, 1.0);
        assertTrue(
            PillBuffHudPlanner.activeBuffs().isEmpty(),
            "negative remaining ticks should not be stored as active"
        );
    }
}
