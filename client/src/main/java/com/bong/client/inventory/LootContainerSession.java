package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryModel;

import java.util.List;
import java.util.Objects;

/** inventory UI 使用的库无关搜刮会话投影。 */
public sealed interface LootContainerSession permits LootContainerSession.Open, LootContainerSession.Closed {
    record Open(
        long sessionId,
        String sourceKind,
        String grade,
        int rows,
        int cols,
        long timeoutWallSecs,
        List<InventoryModel.GridEntry> placedItems
    ) implements LootContainerSession {
        public Open {
            sourceKind = Objects.requireNonNull(sourceKind, "source kind must not be null");
            grade = Objects.requireNonNull(grade, "grade must not be null");
            placedItems = List.copyOf(Objects.requireNonNull(placedItems, "placed items must not be null"));
        }
    }

    record Closed(long sessionId, String reason) implements LootContainerSession {
        public Closed {
            reason = reason == null ? "" : reason;
        }
    }
}
