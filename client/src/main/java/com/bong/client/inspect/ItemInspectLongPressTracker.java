package com.bong.client.inspect;

import com.bong.client.inventory.model.InventoryItem;

public final class ItemInspectLongPressTracker {
    public static final long LONG_PRESS_MS = 1_000L;
    private static final double CANCEL_DISTANCE_SQUARED = 36.0;

    private InventoryItem item;
    private long startedAtMs;
    private double startX;
    private double startY;
    private boolean consumed;

    public void start(InventoryItem nextItem, double mouseX, double mouseY, long nowMs) {
        if (nextItem == null || nextItem.isEmpty()) {
            cancel();
            return;
        }
        item = nextItem;
        startedAtMs = Math.max(0L, nowMs);
        startX = mouseX;
        startY = mouseY;
        consumed = false;
    }

    public void move(double mouseX, double mouseY) {
        if (item == null) {
            return;
        }
        double dx = mouseX - startX;
        double dy = mouseY - startY;
        if (dx * dx + dy * dy > CANCEL_DISTANCE_SQUARED) {
            cancel();
        }
    }

    public InventoryItem consumeReady(long nowMs) {
        if (item == null || consumed || Math.max(0L, nowMs) - startedAtMs < LONG_PRESS_MS) {
            return null;
        }
        consumed = true;
        return item;
    }

    public void cancel() {
        item = null;
        startedAtMs = 0L;
        startX = 0.0;
        startY = 0.0;
        consumed = false;
    }
}
