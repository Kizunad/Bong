package com.bong.client.social;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Objects;
import java.util.Optional;

/** 交易邀请的不可变 UI 投影，保留每件物品的 authoritative instance_id。 */
public record TradeOfferScreenViewModel(
    long revision,
    SocialStateStore.TradeOffer offer,
    List<InventoryItem> choices
) {
    public TradeOfferScreenViewModel {
        if (revision < 0L) throw new IllegalArgumentException("revision must be >= 0");
        Objects.requireNonNull(offer, "offer must not be null");
        choices = List.copyOf(Objects.requireNonNull(choices, "choices must not be null"));
    }

    public static List<InventoryItem> collectChoices(InventoryModel model) {
        if (model == null) return List.of();
        ArrayList<InventoryItem> items = new ArrayList<>();
        for (InventoryModel.GridEntry entry : model.gridItems()) {
            addIfUsable(items, entry.item());
        }
        for (InventoryItem item : model.hotbar()) {
            addIfUsable(items, item);
        }
        items.sort(Comparator.comparing(InventoryItem::displayName).thenComparingLong(InventoryItem::instanceId));
        return List.copyOf(items);
    }

    /** 显式 picker 的唯一解析入口，禁止按排序位置或数组下标推断物品。 */
    public static Optional<InventoryItem> findChoice(InventoryModel model, long instanceId) {
        if (instanceId <= 0L) return Optional.empty();
        return collectChoices(model).stream()
            .filter(item -> item.instanceId() == instanceId)
            .findFirst();
    }

    private static void addIfUsable(List<InventoryItem> items, InventoryItem item) {
        if (item != null && !item.isEmpty() && item.instanceId() > 0) items.add(item);
    }
}
