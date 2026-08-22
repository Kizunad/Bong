package com.bong.client.ui;

import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Component;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.Set;
import java.util.function.BiConsumer;
import java.util.function.Function;

/**
 * Owns the ordered identity of a list of owo components and updates it without
 * clearing the host when only row contents changed.
 *
 * <p>Rows are created in a detached collection before a structural update
 * touches the host. This keeps a failed factory from destroying the last
 * committed view. The key sequence is committed only after the corresponding
 * patch or rebuild has completed successfully.</p>
 */
public final class DiffListWidget<T, K, C extends Component> {
    private final FlowLayout rows;
    private final Function<? super T, ? extends K> keyOf;
    private final Function<? super T, ? extends C> createRow;
    private final BiConsumer<? super C, ? super T> patchRow;

    private List<K> committedKeys = List.of();
    private Map<K, C> mountedByKey = Map.of();

    public DiffListWidget(
        FlowLayout rows,
        Function<? super T, ? extends K> keyOf,
        Function<? super T, ? extends C> createRow,
        BiConsumer<? super C, ? super T> patchRow
    ) {
        this.rows = Objects.requireNonNull(rows, "rows must not be null");
        this.keyOf = Objects.requireNonNull(keyOf, "keyOf must not be null");
        this.createRow = Objects.requireNonNull(createRow, "createRow must not be null");
        this.patchRow = Objects.requireNonNull(patchRow, "patchRow must not be null");
    }

    public UpdateResult update(List<? extends T> items) {
        List<? extends T> source = Objects.requireNonNull(items, "items must not be null");
        List<K> nextKeys = validateKeys(source);

        if (committedKeys.equals(nextKeys)) {
            patchMountedRows(source, nextKeys);
            return UpdateResult.PATCHED;
        }

        List<C> nextRows = new ArrayList<>(source.size());
        Map<K, C> nextRowsByKey = new HashMap<>(capacityFor(source.size()));
        for (int index = 0; index < source.size(); index++) {
            T item = source.get(index);
            C row = createRow.apply(item);
            if (row == null) {
                throw new IllegalArgumentException("createRow returned null at index " + index);
            }
            nextRows.add(row);
            nextRowsByKey.put(nextKeys.get(index), row);
        }

        // All factories have succeeded. The host is changed only as one
        // synchronous replacement, and bookkeeping is committed afterwards.
        List<Component> replacement = new ArrayList<>(nextRows);
        replaceChildrenAtomically(replacement);
        committedKeys = nextKeys;
        mountedByKey = Map.copyOf(nextRowsByKey);
        return UpdateResult.REBUILT;
    }

    public List<K> renderedKeys() {
        return committedKeys;
    }

    public Optional<C> rowForKey(K key) {
        return Optional.ofNullable(mountedByKey.get(key));
    }

    public enum UpdateResult { REBUILT, PATCHED }

    private List<K> validateKeys(List<? extends T> items) {
        List<K> keys = new ArrayList<>(items.size());
        Set<K> seen = new HashSet<>(capacityFor(items.size()));
        for (int index = 0; index < items.size(); index++) {
            T item = items.get(index);
            if (item == null) {
                throw new IllegalArgumentException("items must not contain null at index " + index);
            }
            K key = keyOf.apply(item);
            if (key == null) {
                throw new IllegalArgumentException("keyOf returned null at index " + index);
            }
            if (!seen.add(key)) {
                throw new IllegalArgumentException("duplicate key at index " + index + ": " + key);
            }
            keys.add(key);
        }
        return List.copyOf(keys);
    }

    private void patchMountedRows(List<? extends T> items, List<K> keys) {
        for (int index = 0; index < items.size(); index++) {
            C row = mountedByKey.get(keys.get(index));
            if (row == null) {
                throw new IllegalStateException("missing mounted row for committed key " + keys.get(index));
            }
            patchRow.accept(row, items.get(index));
        }
    }

    private void replaceChildrenAtomically(List<Component> replacement) {
        List<Component> previousChildren = List.copyOf(rows.children());
        try {
            rows.clearChildren();
            rows.children(replacement);
        } catch (RuntimeException | Error failure) {
            try {
                rows.clearChildren();
                rows.children(previousChildren);
            } catch (RuntimeException | Error rollbackFailure) {
                failure.addSuppressed(rollbackFailure);
            }
            throw failure;
        }
    }

    private static int capacityFor(int size) {
        return Math.max(16, (int) Math.ceil(size / 0.75d));
    }
}
