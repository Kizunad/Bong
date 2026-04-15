package com.bong.client.combat;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UnifiedEventStreamTest {

    @Test
    void publishAppendsToStream() {
        UnifiedEventStream s = new UnifiedEventStream();
        assertTrue(s.publish(UnifiedEvent.Channel.COMBAT, UnifiedEvent.Priority.P2_NORMAL,
            "wolf", "命中 野狼", 0, 1000L));
        assertEquals(1, s.size());
    }

    @Test
    void sameKeyFoldsWithinWindow() {
        UnifiedEventStream s = new UnifiedEventStream();
        for (int i = 0; i < 5; i++) {
            s.publish(UnifiedEvent.Channel.COMBAT, UnifiedEvent.Priority.P2_NORMAL,
                "wolf", "命中 野狼", 0, 1000L + i * 200);
        }
        assertEquals(1, s.size(), "same key within fold window becomes a single entry");
        UnifiedEvent e = s.snapshot().get(0);
        assertEquals(5, e.foldCount());
        assertTrue(e.displayText().contains("\u00D75"),
            "display text includes ×N: was " + e.displayText());
    }

    @Test
    void foldWindowExpiresAfter1500ms() {
        UnifiedEventStream s = new UnifiedEventStream();
        s.publish(UnifiedEvent.Channel.COMBAT, UnifiedEvent.Priority.P2_NORMAL,
            "wolf", "命中", 0, 0L);
        s.publish(UnifiedEvent.Channel.COMBAT, UnifiedEvent.Priority.P2_NORMAL,
            "wolf", "命中", 0, 2000L);
        assertEquals(2, s.size(), "after fold window, same text creates a fresh entry");
    }

    @Test
    void throttleDropsCombatBeyondEightPerSecond() {
        UnifiedEventStream s = new UnifiedEventStream();
        int accepted = 0;
        for (int i = 0; i < 20; i++) {
            // Distinct sourceTag so we never trigger fold behavior.
            boolean ok = s.publish(
                UnifiedEvent.Channel.COMBAT,
                UnifiedEvent.Priority.P2_NORMAL,
                "src_" + i,
                "hit " + i,
                0,
                500L + i
            );
            if (ok) accepted++;
        }
        assertEquals(8, accepted, "combat channel caps at 8/sec");
    }

    @Test
    void expireRemovesP3AfterLifetime() {
        UnifiedEventStream s = new UnifiedEventStream();
        s.publish(UnifiedEvent.Channel.SYSTEM, UnifiedEvent.Priority.P3_VERBOSE,
            "tick", "灵田 tick", 0, 0L);
        s.publish(UnifiedEvent.Channel.SYSTEM, UnifiedEvent.Priority.P0_CRITICAL,
            "death", "击杀 妖兽", 0, 0L);
        s.expire(3_000L);
        List<UnifiedEvent> left = s.snapshot();
        assertEquals(1, left.size(), "P3 lifetime is 2s, P0 is sticky");
        assertEquals(UnifiedEvent.Priority.P0_CRITICAL, left.get(0).priority());
    }

    @Test
    void socialChannelNotThrottled() {
        UnifiedEventStream s = new UnifiedEventStream();
        for (int i = 0; i < 50; i++) {
            assertTrue(s.publish(UnifiedEvent.Channel.SOCIAL, UnifiedEvent.Priority.P3_VERBOSE,
                "u" + i, "msg" + i, 0, i));
        }
    }

    @Test
    void displayTextCapsAt99Plus() {
        UnifiedEvent e = new UnifiedEvent(
            UnifiedEvent.Channel.COMBAT,
            UnifiedEvent.Priority.P2_NORMAL,
            "s", "t", 0, 0L);
        for (int i = 0; i < 200; i++) {
            e.bumpFold(0L);
        }
        assertTrue(e.displayText().endsWith("\u00D799+"),
            "after >99 bumps, displayText ends with ×99+: " + e.displayText());
    }

    @Test
    void sizeNeverExceedsMaxEntries() {
        UnifiedEventStream s = new UnifiedEventStream();
        // Space SOCIAL events 100ms apart; SOCIAL is not throttled so every one is accepted.
        for (int i = 0; i < 50; i++) {
            s.publish(UnifiedEvent.Channel.SOCIAL, UnifiedEvent.Priority.P2_NORMAL,
                "src" + i, "msg " + i, 0, i * 100L);
        }
        assertTrue(s.size() <= UnifiedEventStream.MAX_ENTRIES,
            "buffer must cap at MAX_ENTRIES (was " + s.size() + ")");
    }
}
