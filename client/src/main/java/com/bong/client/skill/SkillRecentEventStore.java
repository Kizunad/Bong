package com.bong.client.skill;

import java.util.ArrayList;
import java.util.List;

/**
 * 技艺页中列“近期流水”的客户端轻量事件流。
 * 仅镜像已到达客户端的 skill IPC 事件，不承担权威状态职责。
 */
public final class SkillRecentEventStore {
    private static final int MAX_EVENTS = 24;
    private static volatile List<Entry> snapshot = List.of();

    private SkillRecentEventStore() {}

    public record Entry(
        SkillId skill,
        String kind,
        String text,
        long createdAtMs
    ) {
        public Entry {
            text = text == null ? "" : text;
            createdAtMs = Math.max(0L, createdAtMs);
        }
    }

    public static List<Entry> snapshot() {
        return snapshot;
    }

    public static void append(Entry entry) {
        if (entry == null || entry.skill() == null || entry.text().isBlank()) return;
        ArrayList<Entry> next = new ArrayList<>(snapshot.size() + 1);
        next.add(entry);
        next.addAll(snapshot);
        if (next.size() > MAX_EVENTS) {
            next.subList(MAX_EVENTS, next.size()).clear();
        }
        snapshot = List.copyOf(next);
    }

    public static void clearOnDisconnect() {
        snapshot = List.of();
    }

    public static void resetForTests() {
        snapshot = List.of();
    }
}
