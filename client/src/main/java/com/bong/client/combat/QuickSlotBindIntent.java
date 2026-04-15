package com.bong.client.combat;

import java.util.Optional;

/**
 * Client → server quick-slot binding (§10.4). Empty {@code itemId} means
 * &quot;clear the slot&quot;.
 */
public record QuickSlotBindIntent(int slot, Optional<String> itemId) {
    public QuickSlotBindIntent {
        if (slot < 0 || slot >= QuickSlotConfig.SLOT_COUNT) {
            throw new IllegalArgumentException("slot out of range: " + slot);
        }
        if (itemId == null) itemId = Optional.empty();
    }

    public static QuickSlotBindIntent clear(int slot) {
        return new QuickSlotBindIntent(slot, Optional.empty());
    }

    public static QuickSlotBindIntent bind(int slot, String itemId) {
        return new QuickSlotBindIntent(slot, Optional.ofNullable(itemId));
    }
}
