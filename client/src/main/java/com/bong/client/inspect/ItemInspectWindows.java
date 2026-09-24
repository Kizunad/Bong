package com.bong.client.inspect;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.window.UiWindowDefinition;
import com.bong.client.ui.window.UiWindowManager;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.List;
import java.util.Objects;
import java.util.Set;

/** 全部物品窗共享一次每 tick 的库存读取；绘制位置不拥有状态源。 */
public final class ItemInspectWindows {
    public static final UiWindowDefinition DEFINITION = new UiWindowDefinition(
        "item-inspect", "item-inspect", 180, 140, Set.of(UiWindowDefinition.Capability.WINDOW));
    private final UiWindowManager manager;
    private final UiStateSource<InventoryModel> inventory;
    private final UiStateSource<List<InventoryItem>> externalItems;
    private final Map<UiWindowManager.WindowKey, InventoryItem> items = new LinkedHashMap<>();

    public ItemInspectWindows(UiWindowManager manager, UiStateSource<InventoryModel> inventory,
                              UiStateSource<List<InventoryItem>> externalItems) {
        this.manager = Objects.requireNonNull(manager);
        this.inventory = Objects.requireNonNull(inventory);
        this.externalItems = Objects.requireNonNull(externalItems);
    }

    public UiWindowManager.WindowState open(long instanceId, UiWindowManager.Rect bounds) {
        InventoryItem item = find(inventory.snapshot(), externalItems.snapshot(), instanceId);
        if (item == null) return null;
        var key = manager.key(DEFINITION.windowType(), Long.toString(instanceId));
        var state = manager.openOrFocus(DEFINITION, key, bounds);
        if (!items.containsKey(key)) {
            items.put(key, item);
            state.scope().addCleanup(() -> items.remove(key));
        }
        return state;
    }

    public InventoryItem item(UiWindowManager.WindowKey key) {
        return items.get(key);
    }

    public void refresh() {
        if (items.isEmpty()) return;
        InventoryModel snapshot = inventory.snapshot();
        List<InventoryItem> external = externalItems.snapshot();
        for (var entry : Map.copyOf(items).entrySet()) {
            InventoryItem current = find(snapshot, external, entry.getValue().instanceId());
            if (current == null) manager.close(entry.getKey());
            else items.put(entry.getKey(), current);
        }
    }

    private static InventoryItem find(InventoryModel model, List<InventoryItem> external, long id) {
        if (id == 0L) return null;
        for (var entry : model.gridItems()) if (matches(entry.item(), id)) return entry.item();
        for (var slot : model.equippedSlots().values()) {
            if (matches(slot.held(), id)) return slot.held();
            for (var item : slot.worn()) if (matches(item, id)) return item;
        }
        for (var item : model.hotbar()) if (matches(item, id)) return item;
        for (var item : external) if (matches(item, id)) return item;
        return null;
    }

    private static boolean matches(InventoryItem item, long id) {
        return item != null && !item.isEmpty() && item.instanceId() == id;
    }
}
