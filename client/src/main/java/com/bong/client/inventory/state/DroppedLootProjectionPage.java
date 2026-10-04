package com.bong.client.inventory.state;

import java.util.List;
import java.util.Objects;

/**
 * One server page for a dropped-loot projection.
 *
 * <p>{@code snapshotRevision} and {@code binding} are echoed on every page so
 * the client can reject a page from another context before it reaches a store.</p>
 */
public record DroppedLootProjectionPage(
    long snapshotRevision,
    int pageIndex,
    int pageCount,
    DroppedLootProjectionBinding binding,
    List<DroppedItemStore.Entry> entries
) {
    public DroppedLootProjectionPage {
        if (snapshotRevision < 0) {
            throw new IllegalArgumentException("snapshotRevision must be non-negative");
        }
        if (pageCount <= 0) {
            throw new IllegalArgumentException("pageCount must be positive");
        }
        if (pageIndex < 0 || pageIndex >= pageCount) {
            throw new IllegalArgumentException("pageIndex must be within pageCount");
        }
        Objects.requireNonNull(binding, "binding");
        entries = List.copyOf(Objects.requireNonNull(entries, "entries"));
    }
}
