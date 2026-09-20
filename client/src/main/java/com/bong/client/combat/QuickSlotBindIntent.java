package com.bong.client.combat;

import java.util.Optional;

/**
 * Client → server quick-slot binding (§10.4). Empty {@code instanceId} means
 * &quot;clear the slot&quot;.
 */
public record QuickSlotBindIntent(int slot, Optional<Long> instanceId) {
    public QuickSlotBindIntent {
        if (slot < 0 || slot >= QuickSlotConfig.SLOT_COUNT) {
            throw new IllegalArgumentException("slot out of range: " + slot);
        }
        if (instanceId == null) instanceId = Optional.empty();
    }

    public static QuickSlotBindIntent clear(int slot) {
        return new QuickSlotBindIntent(slot, Optional.empty());
    }

    public static QuickSlotBindIntent bind(int slot, long instanceId) {
        return new QuickSlotBindIntent(slot, Optional.ofNullable(instanceId));
    }
}
