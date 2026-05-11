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

        assertEquals(1, near.size());
        assertEquals(SilentSignalSystem.SignalKind.PEACE_TORCH, near.get(0).kind());
        assertEquals("torch", near.get(0).iconItemId());
        assertTrue(far.isEmpty(), "15 格外的动作不应被当成近距沉默信号");
    }

    @Test
    void fake_signal_no_ui_warning() {
        SilentSignalSystem.ActionSnapshot fakePeace = snapshot(3.0, "minecraft:torch", null, true);

        List<SilentSignalSystem.SilentSignal> signals = SilentSignalSystem.detect(fakePeace);

        assertEquals(1, signals.size());
        assertEquals(SilentSignalSystem.SignalKind.PEACE_TORCH, signals.get(0).kind());
        assertFalse(signals.get(0).explicitWarning());
        assertFalse(SilentSignalSystem.shouldShowRuleExplanation(signals.get(0)));
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

        assertTrue(kinds.contains(SilentSignalSystem.SignalKind.BONE_COIN_OFFER));
        assertTrue(kinds.contains(SilentSignalSystem.SignalKind.SLOW_BACK_AWAY));
        assertTrue(kinds.contains(SilentSignalSystem.SignalKind.DOUBLE_CROUCH_WARNING));
        assertTrue(kinds.contains(SilentSignalSystem.SignalKind.DIRECTION_POINT));
        assertTrue(kinds.contains(SilentSignalSystem.SignalKind.SEATED_NEUTRAL));
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
