package com.bong.client.inventory.model;

import java.util.List;
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
    private final String scrollKind;
    private final String scrollSkillId;
    private final int scrollXpGrant;
    private final Double forgeQuality;
    private final String forgeColor;
    private final List<String> forgeSideEffects;
    private final Integer forgeAchievedTier;
    private final List<String> alchemyLines;

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
        double durability,
        String scrollKind,
        String scrollSkillId,
        int scrollXpGrant,
        Double forgeQuality,
        String forgeColor,
        List<String> forgeSideEffects,
        Integer forgeAchievedTier,
        List<String> alchemyLines
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
        this.scrollKind = scrollKind == null ? "" : scrollKind;
        this.scrollSkillId = scrollSkillId == null ? "" : scrollSkillId;
        this.scrollXpGrant = Math.max(0, scrollXpGrant);
        this.forgeQuality = forgeQuality == null ? null : clamp01(forgeQuality);
        this.forgeColor = forgeColor == null ? "" : forgeColor.trim();
        this.forgeSideEffects = forgeSideEffects == null
            ? List.of()
            : List.copyOf(forgeSideEffects.stream()
                .filter(Objects::nonNull)
                .map(String::trim)
                .filter(value -> !value.isEmpty())
                .toList());
        this.forgeAchievedTier = forgeAchievedTier == null ? null : Math.max(1, Math.min(4, forgeAchievedTier));
        this.alchemyLines = alchemyLines == null
            ? List.of()
            : List.copyOf(alchemyLines.stream()
                .filter(Objects::nonNull)
                .map(String::trim)
                .filter(value -> !value.isEmpty())
                .toList());
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
        return createFullWithAlchemyMeta(
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
            1.0,
            "",
            "",
            0,
            null,
            "",
            List.of(),
            null,
            List.of()
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
            durability,
            "",
            "",
            0,
            null,
            "",
            List.of(),
            null,
            List.of()
        );
    }

    public static InventoryItem createFullWithScrollMeta(
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
        double durability,
        String scrollKind,
        String scrollSkillId,
        int scrollXpGrant
    ) {
        return createFullWithAlchemyMeta(
            instanceId,
            itemId,
            displayName,
            gridWidth,
            gridHeight,
            weight,
            rarity,
            description,
            stackCount,
            spiritQuality,
            durability,
            scrollKind,
            scrollSkillId,
            scrollXpGrant,
            null,
            "",
            List.of(),
            null,
            List.of()
        );
    }

    public static InventoryItem createFullWithForgeMeta(
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
        double durability,
        String scrollKind,
        String scrollSkillId,
        int scrollXpGrant,
        Double forgeQuality,
        String forgeColor,
        List<String> forgeSideEffects,
        Integer forgeAchievedTier
    ) {
        return createFullWithAlchemyMeta(
            instanceId,
            itemId,
            displayName,
            gridWidth,
            gridHeight,
            weight,
            rarity,
            description,
            stackCount,
            spiritQuality,
            durability,
            scrollKind,
            scrollSkillId,
            scrollXpGrant,
            forgeQuality,
            forgeColor,
            forgeSideEffects,
            forgeAchievedTier,
            List.of()
        );
    }

    public static InventoryItem createFullWithAlchemyMeta(
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
        double durability,
        String scrollKind,
        String scrollSkillId,
        int scrollXpGrant,
        Double forgeQuality,
        String forgeColor,
        List<String> forgeSideEffects,
        Integer forgeAchievedTier,
        List<String> alchemyLines
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
            durability,
            scrollKind == null ? "" : scrollKind.trim(),
            scrollSkillId == null ? "" : scrollSkillId.trim(),
            scrollXpGrant,
            forgeQuality,
            forgeColor,
            forgeSideEffects,
            forgeAchievedTier,
            alchemyLines
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

    public String scrollKind() {
        return scrollKind;
    }

    public String scrollSkillId() {
        return scrollSkillId;
    }

    public int scrollXpGrant() {
        return scrollXpGrant;
    }

    public Double forgeQuality() {
        return forgeQuality;
    }

    public String forgeColor() {
        return forgeColor;
    }

    public List<String> forgeSideEffects() {
        return forgeSideEffects;
    }

    public Integer forgeAchievedTier() {
        return forgeAchievedTier;
    }

    public List<String> alchemyLines() {
        return alchemyLines;
    }

    public boolean isSkillScroll() {
        return "skill_scroll".equals(scrollKind);
    }

    public boolean isInscriptionScroll() {
        return "inscription_scroll".equals(scrollKind) || itemId.startsWith("inscription_scroll_");
    }

    public boolean isBoneCoin() {
        return itemId.startsWith("bone_coin_")
            || "fengling_bone_coin".equals(itemId)
            || "rotten_bone_coin".equals(itemId);
    }

    public String inscriptionId() {
        if (!isInscriptionScroll()) return "";
        String prefix = "inscription_scroll_";
        if (itemId.startsWith(prefix) && itemId.length() > prefix.length()) {
            return itemId.substring(prefix.length()).trim();
        }
        return scrollSkillId.trim();
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
            && scrollXpGrant == other.scrollXpGrant
            && Double.compare(weight, other.weight) == 0
            && Double.compare(spiritQuality, other.spiritQuality) == 0
            && Double.compare(durability, other.durability) == 0
            && itemId.equals(other.itemId)
            && displayName.equals(other.displayName)
            && rarity.equals(other.rarity)
            && description.equals(other.description)
            && scrollKind.equals(other.scrollKind)
            && scrollSkillId.equals(other.scrollSkillId)
            && Objects.equals(forgeQuality, other.forgeQuality)
            && forgeColor.equals(other.forgeColor)
            && forgeSideEffects.equals(other.forgeSideEffects)
            && Objects.equals(forgeAchievedTier, other.forgeAchievedTier)
            && alchemyLines.equals(other.alchemyLines);
    }

    @Override
    public int hashCode() {
        return Objects.hash(
            instanceId, itemId, displayName, gridWidth, gridHeight, weight,
            rarity, description, stackCount, spiritQuality, durability,
            scrollKind, scrollSkillId, scrollXpGrant, forgeQuality, forgeColor,
            forgeSideEffects, forgeAchievedTier, alchemyLines
        );
    }

    @Override
    public String toString() {
        return "InventoryItem[" + itemId + " " + gridWidth + "x" + gridHeight
            + " x" + stackCount + " q=" + spiritQuality + "]";
    }
}
