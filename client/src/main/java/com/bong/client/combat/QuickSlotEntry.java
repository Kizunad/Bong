package com.bong.client.combat;

import java.util.Objects;

/**
 * Single F1–F9 quick-use slot binding (§10.4 {@code QuickSlotEntry} IPC
 * payload).
 */
public final class QuickSlotEntry {
    private final String itemId;
    private final String displayName;
    private final int castDurationMs;
    private final int cooldownMs;
    private final String iconTexture;

    public QuickSlotEntry(
        String itemId,
        String displayName,
        int castDurationMs,
        int cooldownMs,
        String iconTexture
    ) {
        this.itemId = Objects.requireNonNull(itemId, "itemId");
        this.displayName = displayName == null ? "" : displayName;
        this.castDurationMs = Math.max(0, castDurationMs);
        this.cooldownMs = Math.max(0, cooldownMs);
        this.iconTexture = iconTexture == null ? "" : iconTexture;
    }

    public String itemId() { return itemId; }
    public String displayName() { return displayName; }
    public int castDurationMs() { return castDurationMs; }
    public int cooldownMs() { return cooldownMs; }
    public String iconTexture() { return iconTexture; }
}
