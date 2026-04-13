package com.bong.client.inventory.model;

import java.util.Objects;

public final class InventoryItem {
    private final long instanceId;
    private final String itemId;
    private final String displayName;
    private final int gridWidth;
    private final int gridHeight;
    private final double weight;
    private final String rarity;
    private final String description;
    private final int stackCount;
    private final double spiritQuality;   // 0..1，< 1.0 暗示已流失灵气
    private final double durability;      // 0..1，< 1.0 暗示损耗

    private InventoryItem(
        long instanceId,
        String itemId,
        String displayName,
        int gridWidth,
        int gridHeight,
        double weight,
        String rarity,
        String description,
        int stackCount,
        double spiritQuality,
        double durability
    ) {
        this.instanceId = instanceId;
        this.itemId = Objects.requireNonNull(itemId, "itemId");
        this.displayName = Objects.requireNonNull(displayName, "displayName");
        this.gridWidth = Math.max(1, Math.min(4, gridWidth));
        this.gridHeight = Math.max(1, Math.min(4, gridHeight));
        this.weight = Math.max(0.0, weight);
        this.rarity = Objects.requireNonNull(rarity, "rarity");
        this.description = Objects.requireNonNull(description, "description");
        this.stackCount = Math.max(1, stackCount);
        this.spiritQuality = clamp01(spiritQuality);
        this.durability = clamp01(durability);
    }

    private static double clamp01(double v) {
        if (Double.isNaN(v)) return 1.0;
        return Math.max(0.0, Math.min(1.0, v));
    }

    /** 旧 7 参签名 —— 默认 stack=1 / quality=1 / durability=1 / instanceId=0。 */
    public static InventoryItem create(
        String itemId,
        String displayName,
        int gridWidth,
        int gridHeight,
        double weight,
        String rarity,
        String description
    ) {
        return createFull(
            0L,
            itemId,
            displayName,
            gridWidth,
            gridHeight,
            weight,
            rarity,
            description,
            1,
            1.0,
            1.0
        );
    }

    /** 新完整签名 —— server snapshot 直用。 */
    public static InventoryItem createFull(
        long instanceId,
        String itemId,
        String displayName,
        int gridWidth,
        int gridHeight,
        double weight,
        String rarity,
        String description,
        int stackCount,
        double spiritQuality,
        double durability
    ) {
        return new InventoryItem(
            instanceId,
            itemId == null ? "" : itemId.trim(),
            displayName == null ? "" : displayName.trim(),
            gridWidth,
            gridHeight,
            weight,
            rarity == null ? "common" : rarity.trim(),
            description == null ? "" : description.trim(),
            stackCount,
            spiritQuality,
            durability
        );
    }

    public static InventoryItem simple(String itemId, String displayName) {
        return create(itemId, displayName, 1, 1, 0.5, "common", "");
    }

    public long instanceId() {
        return instanceId;
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

    public int stackCount() {
        return stackCount;
    }

    public double spiritQuality() {
        return spiritQuality;
    }

    public double durability() {
        return durability;
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
        return instanceId == other.instanceId
            && gridWidth == other.gridWidth
            && gridHeight == other.gridHeight
            && stackCount == other.stackCount
            && Double.compare(weight, other.weight) == 0
            && Double.compare(spiritQuality, other.spiritQuality) == 0
            && Double.compare(durability, other.durability) == 0
            && itemId.equals(other.itemId)
            && displayName.equals(other.displayName)
            && rarity.equals(other.rarity)
            && description.equals(other.description);
    }

    @Override
    public int hashCode() {
        return Objects.hash(
            instanceId, itemId, displayName, gridWidth, gridHeight, weight,
            rarity, description, stackCount, spiritQuality, durability
        );
    }

    @Override
    public String toString() {
        return "InventoryItem[" + itemId + " " + gridWidth + "x" + gridHeight
            + " x" + stackCount + " q=" + spiritQuality + "]";
    }
}
