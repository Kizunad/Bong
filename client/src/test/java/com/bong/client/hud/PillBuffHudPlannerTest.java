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

    @Test
    void nullBuffIdIgnored() {
        PillBuffHudPlanner.updateBuff(null, 3000, 1.0);
        assertTrue(
            PillBuffHudPlanner.activeBuffs().isEmpty(),
            "null buffId should be silently ignored"
        );
    }

    @Test
    void blankBuffIdIgnored() {
        PillBuffHudPlanner.updateBuff("  ", 3000, 1.0);
        assertTrue(
            PillBuffHudPlanner.activeBuffs().isEmpty(),
            "blank buffId should be silently ignored"
        );
    }

    @Test
    void nanEffectMultiplierIgnored() {
        PillBuffHudPlanner.updateBuff("bad_nan", 3000, Double.NaN);
        assertTrue(
            PillBuffHudPlanner.activeBuffs().isEmpty(),
            "NaN effectMultiplier should be silently ignored"
        );
    }

    @Test
    void infiniteEffectMultiplierIgnored() {
        PillBuffHudPlanner.updateBuff("bad_inf", 3000, Double.POSITIVE_INFINITY);
        assertTrue(
            PillBuffHudPlanner.activeBuffs().isEmpty(),
            "infinite effectMultiplier should be silently ignored"
        );
    }

    @Test
    void zeroEffectMultiplierIgnored() {
        PillBuffHudPlanner.updateBuff("bad_zero", 3000, 0.0);
        assertTrue(
            PillBuffHudPlanner.activeBuffs().isEmpty(),
            "zero effectMultiplier should be silently ignored"
        );
    }

    @Test
    void negativeEffectMultiplierIgnored() {
        PillBuffHudPlanner.updateBuff("bad_neg", 3000, -0.5);
        assertTrue(
            PillBuffHudPlanner.activeBuffs().isEmpty(),
            "negative effectMultiplier should be silently ignored"
        );
    }

    @Test
    void progressAtTypicalMaxTicks() {
        PillBuffHudPlanner.PillBuff atMax = new PillBuffHudPlanner.PillBuff("max", 6000, 1.0);
        assertEquals(1.0f, atMax.progress(), 1e-6f,
            "progress at TYPICAL_MAX_TICKS (6000) should be exactly 1.0");
    }

    @Test
    void invalidBuffIdDoesNotRemoveExisting() {
        PillBuffHudPlanner.updateBuff("existing", 3000, 1.0);
        PillBuffHudPlanner.updateBuff(null, 1000, 1.0);
        PillBuffHudPlanner.updateBuff("  ", 1000, 1.0);
        assertEquals(1, PillBuffHudPlanner.activeBuffs().size(),
            "invalid buffId updates should not affect existing buffs");
        assertEquals("existing", PillBuffHudPlanner.activeBuffs().get(0).buffId());
    }
}
