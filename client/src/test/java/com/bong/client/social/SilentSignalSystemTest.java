package com.bong.client.social;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SilentSignalSystemTest {
    @Test
    void signal_detection_range() {
        SilentSignalSystem.ActionSnapshot nearTorch = snapshot(15.0, "minecraft:torch", null, false);
        SilentSignalSystem.ActionSnapshot farTorch = snapshot(15.1, "minecraft:torch", null, false);

        List<SilentSignalSystem.SilentSignal> near = SilentSignalSystem.detect(nearTorch);
        List<SilentSignalSystem.SilentSignal> far = SilentSignalSystem.detect(farTorch);

        assertEquals(
            1,
            near.size(),
            "expected exactly one signal because torch is within 15.0 blocks, actual " + near.size()
        );
        assertEquals(
            SilentSignalSystem.SignalKind.PEACE_TORCH,
            near.get(0).kind(),
            "expected PEACE_TORCH because held item is torch, actual " + near.get(0).kind()
        );
        assertEquals(
            "torch",
            near.get(0).iconItemId(),
            "expected torch icon because held item is minecraft:torch, actual " + near.get(0).iconItemId()
        );
        assertTrue(far.isEmpty(), "expected empty because distance is above 15 blocks, actual " + far);
    }

    @Test
    void fake_signal_no_ui_warning() {
        SilentSignalSystem.ActionSnapshot fakePeace = snapshot(3.0, "minecraft:torch", null, true);

        List<SilentSignalSystem.SilentSignal> signals = SilentSignalSystem.detect(fakePeace);

        assertEquals(
            1,
            signals.size(),
            "expected one signal because fake peace still starts with a visible torch, actual " + signals.size()
        );
        assertEquals(
            SilentSignalSystem.SignalKind.PEACE_TORCH,
            signals.get(0).kind(),
            "expected PEACE_TORCH because held item is torch, actual " + signals.get(0).kind()
        );
        assertFalse(
            signals.get(0).explicitWarning(),
            "expected explicitWarning=false because fake-signal warning is intentionally disabled, actual true"
        );
        assertFalse(
            SilentSignalSystem.shouldShowRuleExplanation(signals.get(0)),
            "expected no rule explanation because silent signals must stay implicit, actual true"
        );
    }

    @Test
    void detects_back_away_crouch_point_and_meditation_without_text() {
        SilentSignalSystem.ActionSnapshot snapshot = new SilentSignalSystem.ActionSnapshot(
            "char:other",
            8.0,
            null,
            "bong:bone_coin",
            true,
            0.5,
            true,
            2,
            2_000L,
            3_000L,
            false
        );

        List<SilentSignalSystem.SignalKind> kinds = SilentSignalSystem.detect(snapshot)
            .stream()
            .map(SilentSignalSystem.SilentSignal::kind)
            .toList();

        assertTrue(
            kinds.contains(SilentSignalSystem.SignalKind.BONE_COIN_OFFER),
            "expected BONE_COIN_OFFER because dropped item is bong:bone_coin, actual " + kinds
        );
        assertTrue(
            kinds.contains(SilentSignalSystem.SignalKind.SLOW_BACK_AWAY),
            "expected SLOW_BACK_AWAY because target is backing away at 0.5x while facing, actual " + kinds
        );
        assertTrue(
            kinds.contains(SilentSignalSystem.SignalKind.DOUBLE_CROUCH_WARNING),
            "expected DOUBLE_CROUCH_WARNING because crouch toggles reached 2, actual " + kinds
        );
        assertTrue(
            kinds.contains(SilentSignalSystem.SignalKind.DIRECTION_POINT),
            "expected DIRECTION_POINT because pointing lasted 2000ms, actual " + kinds
        );
        assertTrue(
            kinds.contains(SilentSignalSystem.SignalKind.SEATED_NEUTRAL),
            "expected SEATED_NEUTRAL because meditation lasted 3000ms, actual " + kinds
        );
    }

    @Test
    void invalid_distance_returns_empty() {
        for (double distance : new double[] {-1.0, Double.NaN, Double.POSITIVE_INFINITY}) {
            List<SilentSignalSystem.SilentSignal> signals =
                SilentSignalSystem.detect(snapshot(distance, "minecraft:torch", null, false));

            assertTrue(
                signals.isEmpty(),
                "expected empty because distance must be finite and non-negative, actual " + signals
            );
        }
    }

    @Test
    void null_snapshot_returns_empty() {
        List<SilentSignalSystem.SilentSignal> signals = SilentSignalSystem.detect(null);

        assertTrue(signals.isEmpty(), "expected empty because snapshot is null, actual " + signals);
    }

    @Test
    void thresholds_off_by_one_do_not_trigger() {
        SilentSignalSystem.ActionSnapshot snapshot = new SilentSignalSystem.ActionSnapshot(
            "char:other",
            8.0,
            null,
            null,
            true,
            0.51,
            true,
            1,
            1_999L,
            2_999L,
            false
        );

        List<SilentSignalSystem.SignalKind> kinds = SilentSignalSystem.detect(snapshot)
            .stream()
            .map(SilentSignalSystem.SilentSignal::kind)
            .toList();

        assertFalse(
            kinds.contains(SilentSignalSystem.SignalKind.SLOW_BACK_AWAY),
            "expected no SLOW_BACK_AWAY because speed is above 0.5x, actual " + kinds
        );
        assertFalse(
            kinds.contains(SilentSignalSystem.SignalKind.DOUBLE_CROUCH_WARNING),
            "expected no DOUBLE_CROUCH_WARNING because crouch toggles are below 2, actual " + kinds
        );
        assertFalse(
            kinds.contains(SilentSignalSystem.SignalKind.DIRECTION_POINT),
            "expected no DIRECTION_POINT because pointing lasted below 2000ms, actual " + kinds
        );
        assertFalse(
            kinds.contains(SilentSignalSystem.SignalKind.SEATED_NEUTRAL),
            "expected no SEATED_NEUTRAL because meditation lasted below 3000ms, actual " + kinds
        );
    }

    @Test
    void invalid_speed_multiplier_does_not_trigger_back_away() {
        for (double speedMultiplier : new double[] {
            -1.0,
            Double.NaN,
            Double.POSITIVE_INFINITY,
            Double.NEGATIVE_INFINITY
        }) {
            SilentSignalSystem.ActionSnapshot snapshot = new SilentSignalSystem.ActionSnapshot(
                "char:other",
                8.0,
                null,
                null,
                true,
                speedMultiplier,
                true,
                0,
                0,
                0,
                false
            );

            List<SilentSignalSystem.SignalKind> kinds = SilentSignalSystem.detect(snapshot)
                .stream()
                .map(SilentSignalSystem.SilentSignal::kind)
                .toList();

            assertFalse(
                kinds.contains(SilentSignalSystem.SignalKind.SLOW_BACK_AWAY),
                "expected no SLOW_BACK_AWAY because speedMultiplier is invalid, actual " + kinds
            );
        }
    }

    private static SilentSignalSystem.ActionSnapshot snapshot(
        double distanceBlocks,
        String heldItemId,
        String droppedItemId,
        boolean followUpAttack
    ) {
        return new SilentSignalSystem.ActionSnapshot(
            "char:other",
            distanceBlocks,
            heldItemId,
            droppedItemId,
            false,
            1.0,
            true,
            0,
            0,
            0,
            followUpAttack
        );
    }
}
