package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InventoryStateStoreTest {

    @AfterEach
    void resetStore() {
        InventoryStateStore.resetForTests();
    }

    @Test
    void distinguishesDisconnectedLoadingAndAuthoritativeLoadedStates() {
        InventoryStateStore.clearOnDisconnect();

        assertFalse(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(-1L, InventoryStateStore.revision());
        assertTrue(InventoryStateStore.snapshot().isEmpty());

        InventoryModel loadingSnapshot = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .build();
        InventoryStateStore.replace(loadingSnapshot);

        assertFalse(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(0L, InventoryStateStore.revision());
        assertSame(loadingSnapshot, InventoryStateStore.snapshot());

        InventoryModel authoritativeSnapshot = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(
                InventoryItem.simple("starter_talisman", "初始护符"),
                InventoryModel.FRONT_SATCHEL_CONTAINER_ID,
                0,
                0
            )
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(authoritativeSnapshot, 12L);

        assertTrue(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(12L, InventoryStateStore.revision());
        assertSame(authoritativeSnapshot, InventoryStateStore.snapshot());
    }

    @Test
    void listenersRemainFunctionalAndCanBeRemovedWithoutLeaks() {
        AtomicInteger callCount = new AtomicInteger();
        AtomicReference<InventoryModel> lastSeen = new AtomicReference<>();
        List<InventoryModel> history = new ArrayList<>();

        java.util.function.Consumer<InventoryModel> listener = snapshot -> {
            callCount.incrementAndGet();
            lastSeen.set(snapshot);
            history.add(snapshot);
        };

        InventoryStateStore.addListener(listener);

        InventoryModel loadingSnapshot = InventoryModel.builder().containers(InventoryModel.DEFAULT_CONTAINERS).build();
        InventoryStateStore.replace(loadingSnapshot);
        assertEquals(1, callCount.get());
        assertSame(loadingSnapshot, lastSeen.get());

        InventoryModel authoritativeSnapshot = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(InventoryItem.simple("starter_talisman", "初始护符"), 0, 0)
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(authoritativeSnapshot, 7L);
        assertEquals(2, callCount.get());
        assertSame(authoritativeSnapshot, lastSeen.get());

        InventoryStateStore.removeListener(listener);
        InventoryStateStore.clearOnDisconnect();

        assertEquals(2, callCount.get());
        assertEquals(List.of(loadingSnapshot, authoritativeSnapshot), history);
    }
}
