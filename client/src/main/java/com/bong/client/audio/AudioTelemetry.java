package com.bong.client.audio;

import java.time.Duration;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.Map;

public final class AudioTelemetry {
    private final Map<String, Deque<Long>> recentPlays = new LinkedHashMap<>();
    private final long windowMs;
    private final int warnThreshold;

    public AudioTelemetry() {
        this(Duration.ofMinutes(30).toMillis(), 100);
    }

    public AudioTelemetry(long windowMs, int warnThreshold) {
        this.windowMs = Math.max(1L, windowMs);
        this.warnThreshold = Math.max(1, warnThreshold);
    }

    public int record(String recipeId, long nowMs) {
        Deque<Long> timestamps = recentPlays.computeIfAbsent(recipeId, ignored -> new ArrayDeque<>());
        timestamps.addLast(nowMs);
        trim(timestamps, nowMs);
        return timestamps.size();
    }

    public boolean isOverThreshold(String recipeId, long nowMs) {
        Deque<Long> timestamps = recentPlays.get(recipeId);
        if (timestamps == null) {
            return false;
        }
        trim(timestamps, nowMs);
        return timestamps.size() > warnThreshold;
    }

    public Map<String, Integer> snapshot(long nowMs) {
        Map<String, Integer> snapshot = new LinkedHashMap<>();
        for (Map.Entry<String, Deque<Long>> entry : recentPlays.entrySet()) {
            trim(entry.getValue(), nowMs);
            if (!entry.getValue().isEmpty()) {
                snapshot.put(entry.getKey(), entry.getValue().size());
            }
        }
        return snapshot;
    }

    private void trim(Deque<Long> timestamps, long nowMs) {
        long cutoff = nowMs - windowMs;
        while (!timestamps.isEmpty() && timestamps.peekFirst() < cutoff) {
            timestamps.removeFirst();
        }
    }
}
