package com.bong.client.craft;

/**
 * plan-craft-v1 §3 + plan-anqi-v2 — 配方分组。
 * 与 server `craft::CraftCategory` / IPC `CraftCategoryV1` 1:1 镜像。
 */
public enum CraftCategory {
    ANQI_CARRIER("anqi_carrier", "暗器载体"),
    DUGU_POTION("dugu_potion", "煎汤 / 自蕴"),
    TUIKE_SKIN("tuike_skin", "伪皮 / 替尸"),
    ZHENFA_TRAP("zhenfa_trap", "阵法预埋件"),
    TOOL("tool", "凡器"),
    CONTAINER("container", "容器 / 装具"),
    POISON_POWDER("poison_powder", "毒粉研磨"),
    MISC("misc", "其它");

    private final String wire;
    private final String displayName;

    CraftCategory(String wire, String displayName) {
        this.wire = wire;
        this.displayName = displayName;
    }

    public String wire() {
        return wire;
    }

    public String displayName() {
        return displayName;
    }

    /** server 推 snake_case；未知 wire fallback 到 MISC（forward-compat）。 */
    public static CraftCategory fromWire(String wire) {
        if (wire == null) return MISC;
        for (CraftCategory c : values()) {
            if (c.wire.equals(wire)) return c;
        }
        return MISC;
    }
}
