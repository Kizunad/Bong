package com.bong.client.combat.store;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Deque;
import java.util.List;
import java.util.concurrent.ConcurrentLinkedDeque;

/**
 * Short-lived list of damage / heal floaters triggered by combat_event payloads
 * (plan §U1). Entries expire after {@link #LIFETIME_MS}. World-space coords are
 * carried so the renderer can project; renderer may choose screen-centered
 * fallback if coords are zero.
 */
public final class DamageFloaterStore {
    public static final long LIFETIME_MS = 1100L;
    public static final int MAX_ENTRIES = 32;

    public enum Kind {
        HIT, CRIT, BLOCK, HEAL, QI_DAMAGE
    }

    public record Floater(
        double worldX,
        double worldY,
        double worldZ,
        String text,
        int color,
        Kind kind,
        long createdAtMs
    ) {
        public Floater {
            text = text == null ? "" : text;
            kind = kind == null ? Kind.HIT : kind;
        }
    }

    private static final Deque<Floater> ENTRIES = new ConcurrentLinkedDeque<>();

    private DamageFloaterStore() {}

    public static void publish(Floater floater) {
        if (floater == null) return;
        ENTRIES.addLast(floater);
        while (ENTRIES.size() > MAX_ENTRIES) ENTRIES.pollFirst();
    }

    public static List<Floater> snapshot(long nowMs) {
        expire(nowMs);
        if (ENTRIES.isEmpty()) return Collections.emptyList();
        return Collections.unmodifiableList(new ArrayList<>(ENTRIES));
    }

    public static void expire(long nowMs) {
        while (true) {
            Floater head = ENTRIES.peekFirst();
            if (head == null) break;
            if (nowMs - head.createdAtMs() <= LIFETIME_MS) break;
            if (!ENTRIES.remove(head)) break;
        }
    }

    public static void resetForTests() {
        ENTRIES.clear();
    }
}
