package com.bong.client.inventory.state;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Collects pages belonging to one exact revision and binding.
 *
 * <p>This is a client-side assembly primitive only. It does not compute a
 * revision, reset token, digest, or context key; those values must come from
 * the server reset/page frames.</p>
 */
public final class DroppedLootPageAssembler {
    /** Result of adding a page without mutating an incompatible assembly. */
    public enum AddResult {
        ACCEPTED_INCOMPLETE,
        ACCEPTED_COMPLETE,
        REJECTED
    }

    private final long snapshotRevision;
    private final DroppedLootProjectionBinding binding;
    private final int pageCount;
    private final Map<Integer, List<DroppedItemStore.Entry>> pages = new LinkedHashMap<>();

    public DroppedLootPageAssembler(DroppedLootProjectionPage firstPage) {
        snapshotRevision = firstPage.snapshotRevision();
        binding = firstPage.binding();
        pageCount = firstPage.pageCount();
        if (add(firstPage) == AddResult.REJECTED) {
            throw new IllegalArgumentException("first page does not match its assembly");
        }
    }

    /** Adds a page only when its revision, binding and page shape match. */
    public AddResult add(DroppedLootProjectionPage page) {
        if (!matches(page)) {
            return AddResult.REJECTED;
        }
        List<DroppedItemStore.Entry> previous = pages.get(page.pageIndex());
        if (previous != null && !previous.equals(page.entries())) {
            return AddResult.REJECTED;
        }
        pages.putIfAbsent(page.pageIndex(), page.entries());
        return isComplete() ? AddResult.ACCEPTED_COMPLETE : AddResult.ACCEPTED_INCOMPLETE;
    }

    public boolean isComplete() {
        return pages.size() == pageCount;
    }

    public long snapshotRevision() {
        return snapshotRevision;
    }

    public DroppedLootProjectionBinding binding() {
        return binding;
    }

    public int pageCount() {
        return pageCount;
    }

    /** Returns pages in server order; incomplete assemblies cannot be committed. */
    public List<DroppedItemStore.Entry> entries() {
        if (!isComplete()) {
            return List.of();
        }
        List<DroppedItemStore.Entry> assembled = new ArrayList<>();
        for (int index = 0; index < pageCount; index++) {
            assembled.addAll(pages.get(index));
        }
        return Collections.unmodifiableList(assembled);
    }

    private boolean matches(DroppedLootProjectionPage page) {
        return page != null
            && page.snapshotRevision() == snapshotRevision
            && page.pageCount() == pageCount
            && page.binding().equals(binding);
    }
}
