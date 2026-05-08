package com.bong.client.craft;

import java.util.Collections;
import java.util.List;
import java.util.Objects;

/**
 * plan-craft-v1 §3 — 客户端配方 DTO。
 *
 * <p>由 server 推的 `craft_recipe_list` payload 反序列化得到。
 * 字段名与 server `CraftRecipeEntryV1` 1:1（snake_case wire ↔ camelCase Java）。</p>
 */
public final class CraftRecipe {
    private final String id;
    private final CraftCategory category;
    private final String displayName;
    private final List<MaterialEntry> materials;
    private final double qiCost;
    private final long timeTicks;
    private final String outputTemplate;
    private final int outputCount;
    private final Requirements requirements;
    private final boolean unlocked;

    public CraftRecipe(
        String id,
        CraftCategory category,
        String displayName,
        List<MaterialEntry> materials,
        double qiCost,
        long timeTicks,
        String outputTemplate,
        int outputCount,
        Requirements requirements,
        boolean unlocked
    ) {
        this.id = Objects.requireNonNull(id, "id");
        this.category = Objects.requireNonNull(category, "category");
        this.displayName = Objects.requireNonNull(displayName, "displayName");
        this.materials = List.copyOf(Objects.requireNonNull(materials, "materials"));
        this.qiCost = qiCost;
        this.timeTicks = timeTicks;
        this.outputTemplate = Objects.requireNonNull(outputTemplate, "outputTemplate");
        this.outputCount = outputCount;
        this.requirements = Objects.requireNonNull(requirements, "requirements");
        this.unlocked = unlocked;
    }

    public String id() { return id; }
    public CraftCategory category() { return category; }
    public String displayName() { return displayName; }
    public List<MaterialEntry> materials() { return materials; }
    public double qiCost() { return qiCost; }
    public long timeTicks() { return timeTicks; }
    public String outputTemplate() { return outputTemplate; }
    public int outputCount() { return outputCount; }
    public Requirements requirements() { return requirements; }
    public boolean unlocked() { return unlocked; }

    /** 仅替换 unlock 字段，其他字段保持。用于 RecipeUnlocked 增量更新。 */
    public CraftRecipe withUnlocked(boolean newUnlocked) {
        if (newUnlocked == this.unlocked) return this;
        return new CraftRecipe(
            id, category, displayName, materials, qiCost, timeTicks,
            outputTemplate, outputCount, requirements, newUnlocked
        );
    }

    /** 单条材料需求 `(template_id, count)`。 */
    public record MaterialEntry(String templateId, int count) {
        public MaterialEntry {
            Objects.requireNonNull(templateId, "templateId");
        }
    }

    /**
     * plan §3 — 配方门槛。任一字段 null 表示该维度无要求。
     * `qiColorRequired = (kindWire, minShare)`。
     */
    public record Requirements(
        String realmMin,
        String qiColorKind,
        Float qiColorMinShare,
        Integer skillLvMin
    ) {
        public static final Requirements NONE = new Requirements(null, null, null, null);

        public boolean hasAny() {
            return realmMin != null
                || qiColorKind != null
                || skillLvMin != null;
        }

        public List<String> humanLines() {
            if (!hasAny()) return Collections.emptyList();
            List<String> lines = new java.util.ArrayList<>(3);
            if (realmMin != null) lines.add("境界 ≥ " + realmMin);
            if (qiColorKind != null) {
                float share = qiColorMinShare == null ? 0f : qiColorMinShare;
                lines.add(String.format("真元色 %s ≥ %.0f%%", qiColorKind, share * 100f));
            }
            if (skillLvMin != null) lines.add("技艺 Lv." + skillLvMin);
            return lines;
        }
    }
}
