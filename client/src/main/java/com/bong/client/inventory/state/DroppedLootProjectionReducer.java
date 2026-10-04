package com.bong.client.inventory.state;

import com.bong.client.network.DroppedLootSyncHandler;

import java.util.ArrayList;
import java.util.List;

/**
 * Fail-closed reducer for the declared dropped-loot reset/page contract.
 *
 * <p>A reset clears the visible projection immediately and raises the revision
 * floor. Pages are committed only after every page for that reset is present and
 * every page repeats the same revision and binding. No method here is wired to
 * {@link DroppedLootSyncHandler}; production cutover belongs to the R6 P3 merge
 * unit.</p>
 */
public final class DroppedLootProjectionReducer {
    /** Observable result of a reset or page attempt. */
    public enum ApplyResult {
        RESET_ACCEPTED,
        PAGE_ACCEPTED_INCOMPLETE,
        PROJECTION_COMMITTED,
        IGNORED_STALE,
        REJECTED
    }

    private long revisionFloor = -1L;
    private long highestCommittedRevision = -1L;
    private long pendingResetRevision = -1L;
    private DroppedLootProjectionBinding activeBinding;
    private DroppedLootPageAssembler assembly;
    private List<DroppedItemStore.Entry> visibleEntries = List.of();

    /** Accepts a strictly newer server reset and clears the current visible view. */
    public ApplyResult applyReset(DroppedLootProjectionReset reset) {
        long revision = reset.projectionRevision();
        if (revision <= Math.max(revisionFloor, highestCommittedRevision)) {
            return ApplyResult.IGNORED_STALE;
        }
        revisionFloor = revision;
        pendingResetRevision = revision;
        activeBinding = reset.binding();
        assembly = null;
        visibleEntries = List.of();
        return ApplyResult.RESET_ACCEPTED;
    }

    /**
     * Accepts a page, or commits a complete replacement projection.
     *
     * <p>When a reset is pending, only its exact revision and binding are valid.
     * Without a pending reset, a complete snapshot may advance the committed
     * revision but cannot change the current context binding.</p>
     */
    public ApplyResult applyPage(DroppedLootProjectionPage page) {
        if (page == null || page.snapshotRevision() < revisionFloor) {
            return ApplyResult.IGNORED_STALE;
        }
        if (page.snapshotRevision() <= highestCommittedRevision) {
            return ApplyResult.IGNORED_STALE;
        }
        if (activeBinding != null && !activeBinding.equals(page.binding())) {
            return ApplyResult.REJECTED;
        }
        if (pendingResetRevision >= 0 && page.snapshotRevision() != pendingResetRevision) {
            return ApplyResult.REJECTED;
        }
        if (assembly == null) {
            assembly = new DroppedLootPageAssembler(page);
        } else if (assembly.snapshotRevision() != page.snapshotRevision()
                || !assembly.binding().equals(page.binding())) {
            return ApplyResult.REJECTED;
        } else if (assembly.add(page) == DroppedLootPageAssembler.AddResult.REJECTED) {
            return ApplyResult.REJECTED;
        }

        if (!assembly.isComplete()) {
            return ApplyResult.PAGE_ACCEPTED_INCOMPLETE;
        }
        List<DroppedItemStore.Entry> assembled = new ArrayList<>(assembly.entries());
        visibleEntries = List.copyOf(assembled);
        highestCommittedRevision = page.snapshotRevision();
        activeBinding = page.binding();
        pendingResetRevision = -1L;
        assembly = null;
        return ApplyResult.PROJECTION_COMMITTED;
    }

    public List<DroppedItemStore.Entry> visibleEntries() {
        return visibleEntries;
    }

    public long revisionFloor() {
        return revisionFloor;
    }

    public long highestCommittedRevision() {
        return highestCommittedRevision;
    }

    public DroppedLootProjectionBinding activeBinding() {
        return activeBinding;
    }
}
