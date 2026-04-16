package com.bong.client.alchemy.state;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * plan-alchemy-v1 §1.3 / §4 — 客户端试药史(LifeRecord.alchemy_attempts 的镜像)。
 *
 * <p>Server 每次结算时推 `alchemy_outcome_resolved`,client 在本 store 累积最近 N 条,
 * inspect / 炼丹 UI 都可读。
 */
public final class AlchemyAttemptHistoryStore {
    public static final int MAX_ENTRIES = 20;

    public record Entry(
        String bucket,        // perfect/good/flawed/waste/explode
        String recipeId,
        String pill,
        String toxinColor,
        String sideEffectTag,
        boolean flawedPath
    ) {
        public Entry {
            if (bucket == null) bucket = "";
            if (recipeId == null) recipeId = "";
            if (pill == null) pill = "";
            if (toxinColor == null) toxinColor = "";
            if (sideEffectTag == null) sideEffectTag = "";
        }
    }

    private static final List<Entry> entries = Collections.synchronizedList(new ArrayList<>());

    private AlchemyAttemptHistoryStore() {}

    public static void append(Entry e) {
        if (e == null) return;
        synchronized (entries) {
            entries.add(e);
            while (entries.size() > MAX_ENTRIES) entries.remove(0);
        }
    }

    public static List<Entry> snapshot() {
        synchronized (entries) {
            return List.copyOf(entries);
        }
    }

    public static void resetForTests() {
        synchronized (entries) { entries.clear(); }
    }
}
