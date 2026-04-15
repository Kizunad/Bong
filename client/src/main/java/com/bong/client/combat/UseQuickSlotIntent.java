package com.bong.client.combat;

/**
 * Client → server intent dispatched when the player presses F1-F9 (§11.3).
 * Slot numbering is 0-based (F1 → 0).
 */
public record UseQuickSlotIntent(int slot) {
    public UseQuickSlotIntent {
        if (slot < 0 || slot >= QuickSlotConfig.SLOT_COUNT) {
            throw new IllegalArgumentException("slot out of range: " + slot);
        }
    }
}
