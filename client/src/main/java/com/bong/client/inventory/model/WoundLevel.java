package com.bong.client.inventory.model;

/**
 * 体表伤势等级。从轻到重，影响外观和功能性。
 */
public enum WoundLevel {
    INTACT("完好",   0xFF44CC66, 1.0),
    BRUISE("淤伤",   0xFF88AA44, 0.95),
    ABRASION("擦伤", 0xFFCCCC44, 0.85),
    LACERATION("割裂", 0xFFCC6644, 0.6),
    FRACTURE("骨折", 0xFFCC4444, 0.3),
    SEVERED("断肢",  0xFF666666, 0.0);

    private final String label;
    private final int color;
    private final double functionRatio; // 功能性系数 0~1

    WoundLevel(String label, int color, double functionRatio) {
        this.label = label;
        this.color = color;
        this.functionRatio = functionRatio;
    }

    public String label() { return label; }
    public int color() { return color; }
    public double functionRatio() { return functionRatio; }

    public boolean isDisabling() { return this == FRACTURE || this == SEVERED; }
    public boolean isSevered() { return this == SEVERED; }
}
