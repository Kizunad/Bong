package com.bong.client.combat;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Deque;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * Bounded FIFO event buffer with per-channel throttling and &ldquo;same key in
 * window&rdquo; folding (§6). Not thread-safe on its own — wrap access through
 * {@link UnifiedEventStore}.
 */
public final class UnifiedEventStream {
    public static final int MAX_ENTRIES = 18;
    public static final long FOLD_WINDOW_MS = 1500L;
    public static final long THROTTLE_WINDOW_MS = 1000L;

    private static final UnifiedEventStream EMPTY = new UnifiedEventStream();

    private final Deque<UnifiedEvent> entries = new ArrayDeque<>(MAX_ENTRIES);
    private final Map<UnifiedEvent.Channel, ArrayDeque<Long>> throttleWindows = new EnumMap<>(UnifiedEvent.Channel.class);

    public static UnifiedEventStream empty() {
        return EMPTY;
    }

    public UnifiedEventStream() {
        // default ctor intentionally left empty.
    }

    /** Per-channel per-second throughput caps (§6.1). */
    public static int perSecondCap(UnifiedEvent.Channel channel) {
        return switch (channel) {
            case COMBAT -> 8;
            case CULTIVATION -> 3;
            case WORLD -> 3;
            case SYSTEM -> 2;
            case SOCIAL -> Integer.MAX_VALUE; // chat goes to the native chat HUD
        };
    }

    /**
     * Publish an event into the stream. Returns true if it was accepted, false
     * if throttled out or dropped.
     */
    public synchronized boolean publish(
        UnifiedEvent.Channel channel,
        UnifiedEvent.Priority priority,
        String sourceTag,
        String text,
        int color,
        long nowMs
    ) {
        if (channel == null || priority == null || text == null) return false;

        // 1) Fold within window
        UnifiedEvent existing = findFoldable(channel, sourceTag, text, nowMs);
        if (existing != null) {
            existing.bumpFold(nowMs);
            return true;
        }

        // 2) Throttle per channel
        if (!acceptThrottle(channel, nowMs)) {
            return false;
        }

        UnifiedEvent e = new UnifiedEvent(channel, priority, sourceTag, text, color, nowMs);
        entries.addLast(e);
        evictIfOverflowing();
        return true;
    }

    private UnifiedEvent findFoldable(
        UnifiedEvent.Channel channel,
        String sourceTag,
        String text,
        long nowMs
    ) {
        for (UnifiedEvent e : entries) {
            if (e.channel() != channel) continue;
            if (!e.sourceTag().equals(sourceTag == null ? "" : sourceTag)) continue;
            if (!e.text().equals(text)) continue;
            if (nowMs - e.lastUpdatedMs() <= FOLD_WINDOW_MS) {
                return e;
            }
        }
        return null;
    }

    private boolean acceptThrottle(UnifiedEvent.Channel channel, long nowMs) {
        int cap = perSecondCap(channel);
        if (cap == Integer.MAX_VALUE) return true;
        ArrayDeque<Long> window = throttleWindows.computeIfAbsent(channel, k -> new ArrayDeque<>());
        while (!window.isEmpty() && nowMs - window.peekFirst() > THROTTLE_WINDOW_MS) {
            window.pollFirst();
        }
        if (window.size() >= cap) {
            return false;
        }
        window.addLast(nowMs);
        return true;
    }

    private void evictIfOverflowing() {
        while (entries.size() > MAX_ENTRIES) {
            // Evict lowest-priority / oldest first
            UnifiedEvent victim = pickEvictionVictim();
            if (victim == null) break;
            entries.remove(victim);
        }
    }

    private UnifiedEvent pickEvictionVictim() {
        UnifiedEvent worst = null;
        for (UnifiedEvent e : entries) {
            if (e.priority() == UnifiedEvent.Priority.P0_CRITICAL) continue;
            if (worst == null
                || e.priority().ordinal() > worst.priority().ordinal()
                || (e.priority() == worst.priority() && e.createdAtMs() < worst.createdAtMs())) {
                worst = e;
            }
        }
        if (worst == null && !entries.isEmpty()) {
            // All P0 — fall back to oldest.
            worst = entries.peekFirst();
        }
        return worst;
    }

    /** Drop entries whose lifetime has elapsed. Call from HUD tick. */
    public synchronized void expire(long nowMs) {
        entries.removeIf(e -> {
            if (e.priority() == UnifiedEvent.Priority.P0_CRITICAL) return false;
            long life = e.priority().lifetimeMs();
            return nowMs - e.lastUpdatedMs() > life;
        });
    }

    public synchronized List<UnifiedEvent> snapshot() {
        return Collections.unmodifiableList(new ArrayList<>(entries));
    }

    public synchronized int size() {
        return entries.size();
    }

    public synchronized void clear() {
        entries.clear();
        throttleWindows.clear();
    }
}
