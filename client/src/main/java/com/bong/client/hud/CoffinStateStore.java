package com.bong.client.hud;

/**
 * 卧棺状态存储（plan-coffin-tiers-v1 P3）。
 *
 * <p>P3 新增 {@code grade} 字段（"mundane"/"jade"/"stone"/"bronze"；null = 不在棺内），
 * 由 {@link com.bong.client.network.CoffinStateHandler} 从 {@code coffin_grade} 字段解析并写入。</p>
 */
public final class CoffinStateStore {
    public static final State OUT = new State(false, 1.0, null);

    private static volatile State snapshot = OUT;

    private CoffinStateStore() {
    }

    public static synchronized State snapshot() {
        return snapshot;
    }

    public static synchronized void replace(State next) {
        snapshot = next == null ? OUT : next.normalized();
    }

    public static synchronized void clear() {
        snapshot = OUT;
    }

    public static void resetForTests() {
        clear();
    }

    /**
     * @param inCoffin              是否卧棺中
     * @param lifespanRateMultiplier 寿元燃烧速率倍率（0 < x <= 1；无效时归一为 1.0）
     * @param grade                 棺材档级字符串（"mundane"/"jade"/"stone"/"bronze"；null = 不在棺内）
     */
    public record State(boolean inCoffin, double lifespanRateMultiplier, String grade) {
        State normalized() {
            double multiplier = Double.isFinite(lifespanRateMultiplier) && lifespanRateMultiplier > 0.0
                ? lifespanRateMultiplier
                : 1.0;
            return new State(inCoffin, multiplier, grade);
        }
    }
}
