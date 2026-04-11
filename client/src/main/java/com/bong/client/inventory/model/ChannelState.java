package com.bong.client.inventory.model;

/**
 * 单条经脉的运行时状态。不可变值对象，由服务端推送或本地模拟生成。
 *
 * <p>使用 record 保证不可变性。构造时自动 clamp 各字段到合法范围。
 */
public record ChannelState(
    MeridianChannel channel,
    double capacity,
    double currentFlow,
    DamageLevel damage,
    double contamination,
    double healProgress,
    boolean blocked
) {

    /** 经脉损伤等级，对应《爆脉流正法》中的撕裂分级。 */
    public enum DamageLevel {
        INTACT("通畅", 0xFF44CC66, 1.0),
        MICRO_TEAR("微裂", 0xFFCCCC44, 0.85),
        TORN("撕裂", 0xFFCC6644, 0.5),
        SEVERED("断脉", 0xFF666666, 0.0);

        private final String label;
        private final int color;
        private final double flowMultiplier;

        DamageLevel(String label, int color, double flowMultiplier) {
            this.label = label;
            this.color = color;
            this.flowMultiplier = flowMultiplier;
        }

        public String label() { return label; }
        public int color() { return color; }
        public double flowMultiplier() { return flowMultiplier; }
    }

    /** Compact constructor — clamp values to valid ranges. */
    public ChannelState {
        capacity = Math.max(0, capacity);
        currentFlow = Math.max(0, Math.min(currentFlow, capacity));
        contamination = Math.max(0, Math.min(1, contamination));
        healProgress = Math.max(0, Math.min(1, healProgress));
    }

    /** 便捷构造：通畅状态，满流量，无污染。 */
    public static ChannelState healthy(MeridianChannel channel, double capacity) {
        return new ChannelState(channel, capacity, capacity, DamageLevel.INTACT, 0, 0, false);
    }

    /** 实际有效流量 = 当前流量 × 损伤系数 × (1 - 污染) */
    public double effectiveFlow() {
        if (blocked) return 0;
        return currentFlow * damage.flowMultiplier() * (1.0 - contamination);
    }

    /** 流量比率 0~1 */
    public double flowRatio() {
        return capacity > 0 ? currentFlow / capacity : 0;
    }
}
