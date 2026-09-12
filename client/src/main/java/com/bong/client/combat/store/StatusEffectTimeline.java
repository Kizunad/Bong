package com.bong.client.combat.store;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Session-owned presentation clock; authoritative effects never wait for animation. */
public final class StatusEffectTimeline {
    public static final long ENTER_MS = 900;
    public static final long HOLD_MS = 340;
    public static final long EXIT_MS = 280;
    public static final long WARNING_MS = 5_000;

    public enum Phase { QUEUED, ENTERING, ACTIVE, EXITING }

    public record Item(StatusEffectStore.Effect effect, Phase phase, long ageMs,
                       long remainingMs, double remainingFraction) {}
    public record Frame(List<Item> items, int waiting) {}

    private final Map<String, Entry> entries = new LinkedHashMap<>();

    public void replace(List<StatusEffectStore.Effect> effects, long nowMs) {
        Map<String, StatusEffectStore.Effect> incoming = new LinkedHashMap<>();
        for (var effect : effects) {
            if (!effect.id().isBlank() && effect.remainingMs() > 0) {
                incoming.merge(effect.id(), effect, (first, next) -> new StatusEffectStore.Effect(
                    first.id(), first.displayName(), first.kind(),
                    (int) Math.min(Integer.MAX_VALUE, (long) first.stacks() + next.stacks()),
                    Math.max(first.remainingMs(), next.remainingMs()), first.sourceColor(),
                    first.sourceLabel(), Math.max(first.dispelDifficulty(), next.dispelDifficulty())));
            }
        }
        var iterator = entries.entrySet().iterator();
        while (iterator.hasNext()) {
            var pair = iterator.next();
            Entry entry = pair.getValue();
            var refreshed = incoming.remove(pair.getKey());
            if (refreshed != null) {
                if (entry.phase == Phase.EXITING) {
                    iterator.remove();
                    incoming.put(pair.getKey(), refreshed);
                    continue;
                }
                entry.effect = refreshed;
                entry.receivedAt = nowMs;
                entry.duration = Math.max(entry.duration, refreshed.remainingMs());
            } else if (entry.phase == Phase.QUEUED || entry.phase == Phase.ENTERING) {
                iterator.remove();
            } else {
                entry.exit(nowMs);
            }
        }
        // Priority orders a simultaneous batch only; refreshed snapshots cannot reorder it.
        incoming.values().stream()
            .sorted(Comparator.comparingInt(e -> StatusEffectStore.rank(e.kind())))
            .forEach(effect -> entries.put(effect.id(), new Entry(effect, nowMs)));
    }

    public Frame frame(long nowMs, int capacity) {
        var iterator = entries.values().iterator();
        while (iterator.hasNext()) {
            Entry entry = iterator.next();
            if (entry.remaining(nowMs) == 0) {
                if (entry.phase == Phase.QUEUED || entry.phase == Phase.ENTERING) {
                    iterator.remove();
                    continue;
                }
                entry.exit(nowMs);
            }
            if (entry.phase == Phase.EXITING && nowMs - entry.startedAt >= EXIT_MS) {
                iterator.remove();
            } else if (entry.phase == Phase.ENTERING && nowMs - entry.startedAt >= ENTER_MS) {
                entry.phase = Phase.ACTIVE;
            }
        }
        int visible = (int) entries.values().stream().filter(e -> e.phase != Phase.QUEUED).count();
        boolean entering = entries.values().stream().anyMatch(e -> e.phase == Phase.ENTERING);
        if (!entering && visible < capacity) {
            for (Entry entry : entries.values()) {
                if (entry.phase == Phase.QUEUED) {
                    entry.phase = Phase.ENTERING;
                    entry.startedAt = nowMs;
                    break;
                }
            }
        }
        List<Item> items = new ArrayList<>();
        int waiting = 0;
        for (Entry entry : entries.values()) {
            if (entry.phase == Phase.QUEUED) {
                waiting++;
            } else {
                long remaining = entry.remaining(nowMs);
                items.add(new Item(entry.effect, entry.phase, Math.max(0, nowMs - entry.startedAt),
                    remaining, Math.min(1, (double) remaining / entry.duration)));
            }
        }
        return new Frame(List.copyOf(items), waiting);
    }

    public void clear() { entries.clear(); }

    private static final class Entry {
        private StatusEffectStore.Effect effect;
        private long receivedAt;
        private long duration;
        private long startedAt;
        private Phase phase = Phase.QUEUED;

        private Entry(StatusEffectStore.Effect effect, long nowMs) {
            this.effect = effect;
            receivedAt = nowMs;
            duration = effect.remainingMs();
        }

        private long remaining(long nowMs) {
            return Math.max(0, effect.remainingMs() - Math.max(0, nowMs - receivedAt));
        }

        private void exit(long nowMs) {
            if (phase != Phase.EXITING) {
                phase = Phase.EXITING;
                startedAt = nowMs;
            }
        }
    }
}
