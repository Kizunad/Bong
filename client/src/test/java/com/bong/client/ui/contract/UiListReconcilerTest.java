package com.bong.client.ui.contract;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

class UiListReconcilerTest {
    @Test
    void emptyItemsPatchAndStructuralChangesAreObservable() {
        UiListReconciler<Item, Integer> reconciler = new UiListReconciler<>(Item::id);
        List<String> operations = new ArrayList<>();
        var patch = (java.util.function.BiConsumer<Integer, Item>) (key, item) -> operations.add("patch:" + key);
        var rebuild = (java.util.function.Consumer<List<UiListReconciler.Entry<Item, Integer>>>) entries ->
            operations.add("rebuild:" + entries.stream().map(UiListReconciler.Entry::key).toList());

        assertEquals(UiListReconciler.UpdateResult.PATCHED, reconciler.update(List.of(), patch, rebuild));
        assertEquals(UiListReconciler.UpdateResult.REBUILT,
            reconciler.update(List.of(new Item(1, "a"), new Item(2, "b")), patch, rebuild));
        assertEquals(UiListReconciler.UpdateResult.PATCHED,
            reconciler.update(List.of(new Item(1, "a2"), new Item(2, "b2")), patch, rebuild));
        assertEquals(UiListReconciler.UpdateResult.REBUILT,
            reconciler.update(List.of(new Item(2, "b3"), new Item(3, "c")), patch, rebuild));
        assertEquals(List.of("rebuild:[1, 2]", "patch:1", "patch:2", "rebuild:[2, 3]"), operations);
        assertEquals(List.of(2, 3), reconciler.committedKeys());
    }

    @Test
    void invalidKeysFailBeforeMutation() {
        AtomicInteger rebuilds = new AtomicInteger();
        UiListReconciler<Item, Integer> reconciler = new UiListReconciler<>(Item::id);
        var patch = (java.util.function.BiConsumer<Integer, Item>) (key, item) -> {
        };
        var rebuild = (java.util.function.Consumer<List<UiListReconciler.Entry<Item, Integer>>>) ignored -> rebuilds.incrementAndGet();

        assertThrows(IllegalArgumentException.class,
            () -> reconciler.update(List.of(new Item(1, "a"), new Item(1, "duplicate")), patch, rebuild));
        assertThrows(IllegalArgumentException.class,
            () -> reconciler.update(Collections.singletonList(null), patch, rebuild));
        UiListReconciler<Item, Integer> nullKey = new UiListReconciler<>(ignored -> null);
        assertThrows(IllegalArgumentException.class,
            () -> nullKey.update(List.of(new Item(1, "a")), patch, rebuild));
        assertEquals(0, rebuilds.get(), "非法输入必须在任何 host mutation 前失败");
    }

    @Test
    void patchFailureKeepsCommittedKeysAndRetriesTheFullList() {
        UiListReconciler<Item, Integer> reconciler = new UiListReconciler<>(Item::id);
        var rebuild = (java.util.function.Consumer<List<UiListReconciler.Entry<Item, Integer>>>) ignored -> {
        };
        reconciler.update(List.of(new Item(1, "a"), new Item(2, "b")),
            (key, item) -> {
            }, rebuild);

        AtomicBoolean failOnce = new AtomicBoolean(true);
        List<Integer> patched = new ArrayList<>();
        assertThrows(IllegalStateException.class, () -> reconciler.update(
            List.of(new Item(1, "a2"), new Item(2, "b2")),
            (key, item) -> {
                patched.add(key);
                if (key == 2 && failOnce.getAndSet(false)) {
                    throw new IllegalStateException("patch failed");
                }
            },
            rebuild
        ));
        assertEquals(List.of(1, 2), reconciler.committedKeys());
        patched.clear();
        assertEquals(UiListReconciler.UpdateResult.PATCHED, reconciler.update(
            List.of(new Item(1, "a2"), new Item(2, "b2")),
            (key, item) -> patched.add(key),
            rebuild
        ));
        assertEquals(List.of(1, 2), patched, "patch 失败后的下一次更新必须从第一行完整重试");
    }

    @Test
    void rebuildFailureDoesNotAdvanceCommittedKeysAndEntriesAreReadOnly() {
        UiListReconciler<Item, Integer> reconciler = new UiListReconciler<>(Item::id);
        reconciler.update(List.of(new Item(1, "a")), (key, item) -> {
        }, ignored -> {
        });
        assertThrows(IllegalStateException.class, () -> reconciler.update(
            List.of(new Item(2, "b")),
            (key, item) -> {
            }, entries -> {
                assertThrows(UnsupportedOperationException.class, () -> entries.add(new UiListReconciler.Entry<>(3, new Item(3, "c"))));
                throw new IllegalStateException("rebuild failed");
            }
        ));
        assertEquals(List.of(1), reconciler.committedKeys());
    }

    private record Item(int id, String label) {
    }
}
