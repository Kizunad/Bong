package com.bong.client.processing.state;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/** plan-lingtian-process-v1 P3 — item_uuid → freshness UI tag 缓存。 */
public final class FreshnessStore {
    public record Entry(String itemUuid, float freshness, String profileName) {}

    private static final Map<String, Entry> ENTRIES = new ConcurrentHashMap<>();

    private FreshnessStore() {}

    public static void upsert(String itemUuid, float freshness, String profileName) {
        if (itemUuid == null || itemUuid.isBlank()) return;
        float clamped = Math.max(0.0f, Math.min(1.0f, freshness));
        ENTRIES.put(itemUuid, new Entry(itemUuid, clamped, profileName == null ? "" : profileName));
    }

    public static Entry get(String itemUuid) {
        return ENTRIES.get(itemUuid);
    }

    public static void clearForTests() {
        ENTRIES.clear();
    }
}
