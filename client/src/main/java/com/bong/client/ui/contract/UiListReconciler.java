package com.bong.client.ui.contract;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.function.BiConsumer;
import java.util.function.Consumer;
import java.util.function.Function;

/**
 * 渲染器无关的有序键协调器。重建回调负责创建脱离宿主的行并必须
 * 原子替换宿主；只有回调成功返回后，已提交键才会推进到新版本。
 */
public final class UiListReconciler<T, K> {
    private final Function<? super T, ? extends K> keyOf;
    private List<K> committedKeys = List.of();

    public UiListReconciler(Function<? super T, ? extends K> keyOf) {
        this.keyOf = Objects.requireNonNull(keyOf, "keyOf must not be null");
    }

    public UpdateResult update(
        List<? extends T> items,
        BiConsumer<? super K, ? super T> patch,
        Consumer<? super List<Entry<T, K>>> rebuild
    ) {
        Objects.requireNonNull(items, "items must not be null");
        Objects.requireNonNull(patch, "patch must not be null");
        Objects.requireNonNull(rebuild, "rebuild must not be null");
        List<K> nextKeys = validateKeys(items);
        if (committedKeys.equals(nextKeys)) {
            for (int index = 0; index < items.size(); index++) {
                patch.accept(nextKeys.get(index), items.get(index));
            }
            return UpdateResult.PATCHED;
        }

        List<Entry<T, K>> entries = new ArrayList<>(items.size());
        for (int index = 0; index < items.size(); index++) {
            entries.add(new Entry<>(nextKeys.get(index), items.get(index)));
        }
        rebuild.accept(List.copyOf(entries));
        committedKeys = List.copyOf(nextKeys);
        return UpdateResult.REBUILT;
    }

    public List<K> committedKeys() {
        return committedKeys;
    }

    private List<K> validateKeys(List<? extends T> items) {
        List<K> keys = new ArrayList<>(items.size());
        Set<K> seen = new HashSet<>();
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

    public record Entry<T, K>(K key, T item) {
        public Entry {
            Objects.requireNonNull(key, "entry key must not be null");
            Objects.requireNonNull(item, "entry item must not be null");
        }
    }

    public enum UpdateResult {
        REBUILT,
        PATCHED
    }
}
