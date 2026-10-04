package com.bong.client.inventory.state;

import java.util.Objects;

/**
 * Server-owned identity binding for one dropped-loot projection revision.
 *
 * <p>The client stores and compares this tuple but never creates or advances it.
 * A context change must therefore arrive as a new server reset.</p>
 */
public record DroppedLootProjectionBinding(
    long projectionEpoch,
    String resetToken,
    String projectionDigest
) {
    public DroppedLootProjectionBinding {
        if (projectionEpoch < 0) {
            throw new IllegalArgumentException("projectionEpoch must be non-negative");
        }
        resetToken = requireText(resetToken, "resetToken");
        projectionDigest = requireText(projectionDigest, "projectionDigest");
    }

    private static String requireText(String value, String field) {
        Objects.requireNonNull(value, field);
        if (value.isBlank()) {
            throw new IllegalArgumentException(field + " must not be blank");
        }
        return value;
    }
}
