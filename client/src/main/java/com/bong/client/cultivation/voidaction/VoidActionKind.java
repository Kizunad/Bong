package com.bong.client.cultivation.voidaction;

public enum VoidActionKind {
    SUPPRESS_TSY("suppress_tsy", "镇压坍缩渊", 200.0, 50, 30L * 24L * 60L * 60L * 20L),
    EXPLODE_ZONE("explode_zone", "引爆区域", 300.0, 100, 90L * 24L * 60L * 60L * 20L),
    BARRIER("barrier", "化虚障", 150.0, 30, 7L * 24L * 60L * 60L * 20L),
    LEGACY_ASSIGN("legacy_assign", "道统传承", 0.0, 0, 0L);

    private final String wireName;
    private final String label;
    private final double qiCost;
    private final int lifespanCostYears;
    private final long cooldownTicks;

    VoidActionKind(String wireName, String label, double qiCost, int lifespanCostYears, long cooldownTicks) {
        this.wireName = wireName;
        this.label = label;
        this.qiCost = qiCost;
        this.lifespanCostYears = lifespanCostYears;
        this.cooldownTicks = cooldownTicks;
    }

    public String wireName() {
        return wireName;
    }

    public String label() {
        return label;
    }

    public double qiCost() {
        return qiCost;
    }

    public int lifespanCostYears() {
        return lifespanCostYears;
    }

    public long cooldownTicks() {
        return cooldownTicks;
    }
}
