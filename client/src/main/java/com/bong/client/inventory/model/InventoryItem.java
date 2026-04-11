package com.bong.client.inventory.model;

import java.util.Objects;

public final class InventoryItem {
    private final String itemId;
    private final String displayName;
    private final int gridWidth;
    private final int gridHeight;
    private final double weight;
    private final String rarity;
    private final String description;

    private InventoryItem(
        String itemId,
        String displayName,
        int gridWidth,
        int gridHeight,
        double weight,
        String rarity,
        String description
    ) {
        this.itemId = Objects.requireNonNull(itemId, "itemId");
        this.displayName = Objects.requireNonNull(displayName, "displayName");
        this.gridWidth = Math.max(1, Math.min(4, gridWidth));
        this.gridHeight = Math.max(1, Math.min(4, gridHeight));
        this.weight = Math.max(0.0, weight);
        this.rarity = Objects.requireNonNull(rarity, "rarity");
        this.description = Objects.requireNonNull(description, "description");
    }

    public static InventoryItem create(
        String itemId,
        String displayName,
        int gridWidth,
        int gridHeight,
        double weight,
        String rarity,
        String description
    ) {
        return new InventoryItem(
            itemId == null ? "" : itemId.trim(),
            displayName == null ? "" : displayName.trim(),
            gridWidth,
            gridHeight,
            weight,
            rarity == null ? "common" : rarity.trim(),
            description == null ? "" : description.trim()
        );
    }

    public static InventoryItem simple(String itemId, String displayName) {
        return create(itemId, displayName, 1, 1, 0.5, "common", "");
    }

    public String itemId() {
        return itemId;
    }

    public String displayName() {
        return displayName;
    }

    public int gridWidth() {
        return gridWidth;
    }

    public int gridHeight() {
        return gridHeight;
    }

    public double weight() {
        return weight;
    }

    public String rarity() {
        return rarity;
    }

    public String description() {
        return description;
    }

    public boolean isEmpty() {
        return itemId.isEmpty();
    }

    public int rarityColor() {
        return switch (rarity) {
            case "legendary" -> 0xFFAA00;
            case "rare" -> 0x5555FF;
            case "uncommon" -> 0x55FF55;
            default -> 0xAAAAAA;
        };
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof InventoryItem other)) return false;
        return gridWidth == other.gridWidth
            && gridHeight == other.gridHeight
            && Double.compare(weight, other.weight) == 0
            && itemId.equals(other.itemId)
            && displayName.equals(other.displayName)
            && rarity.equals(other.rarity)
            && description.equals(other.description);
    }

    @Override
    public int hashCode() {
        return Objects.hash(itemId, displayName, gridWidth, gridHeight, weight, rarity, description);
    }

    @Override
    public String toString() {
        return "InventoryItem[" + itemId + " " + gridWidth + "x" + gridHeight + "]";
    }
}
