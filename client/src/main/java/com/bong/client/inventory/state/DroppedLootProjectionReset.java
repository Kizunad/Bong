package com.bong.client.inventory.state;

import java.util.Objects;

/** Server reset frame that starts a replacement dropped-loot projection. */
public record DroppedLootProjectionReset(
    long projectionRevision,
    DroppedLootProjectionBinding binding
) {
    public DroppedLootProjectionReset {
        if (projectionRevision < 0) {
            throw new IllegalArgumentException("projectionRevision must be non-negative");
        }
        Objects.requireNonNull(binding, "binding");
    }
}
