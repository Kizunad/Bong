package com.bong.client.social;

import com.bong.client.inventory.model.InventoryItem;

import java.util.List;
import java.util.Objects;
import java.util.Optional;

/** 无 UI 库的交易物品选择器，只保留 authoritative instance_id。 */
public final class TradeOfferPicker {
    private List<InventoryItem> choices;
    private long selectedInstanceId = -1L;

    public TradeOfferPicker(List<InventoryItem> choices) {
        update(choices);
    }

    public List<InventoryItem> choices() {
        return choices;
    }

    public long selectedInstanceId() {
        return selectedInstanceId;
    }

    public int selectedIndex() {
        return indexOf(choices, selectedInstanceId);
    }

    /** 库存快照变化时只保留同一个 instance_id，物品消失则清除选择。 */
    public void update(List<InventoryItem> nextChoices) {
        choices = List.copyOf(Objects.requireNonNull(nextChoices, "choices must not be null"));
        if (indexOf(choices, selectedInstanceId) < 0) {
            selectedInstanceId = -1L;
        }
    }

    /** 只有显式移动操作才会产生选择；初始状态不会自动选第一件。 */
    public boolean move(int delta) {
        if (delta == 0 || choices.isEmpty()) return false;
        int current = selectedIndex();
        int next = current < 0
            ? delta < 0 ? choices.size() - 1 : 0
            : Math.floorMod(current + delta, choices.size());
        selectedInstanceId = choices.get(next).instanceId();
        return true;
    }

    /** 在新的 authoritative 快照中按原始 instance_id 重新确认选择。 */
    public Optional<InventoryItem> selectedFrom(List<InventoryItem> authoritativeChoices) {
        if (selectedInstanceId <= 0L || authoritativeChoices == null) return Optional.empty();
        return authoritativeChoices.stream()
            .filter(Objects::nonNull)
            .filter(item -> item.instanceId() == selectedInstanceId)
            .findFirst();
    }

    static int indexOf(List<InventoryItem> choices, long instanceId) {
        if (choices == null || instanceId <= 0L) return -1;
        for (int index = 0; index < choices.size(); index++) {
            InventoryItem item = choices.get(index);
            if (item != null && item.instanceId() == instanceId) return index;
        }
        return -1;
    }
}
