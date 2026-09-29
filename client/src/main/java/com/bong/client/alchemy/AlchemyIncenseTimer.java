package com.bong.client.alchemy;

/** 服务端香状态的只读 UI 投影；客户端不自行计时，也不修改库存或炼丹效果。 */
public final class AlchemyIncenseTimer {
    public enum State { EMPTY, BURNING, SPENT }

    public record Snapshot(
        State state,
        long remainingMillis,
        int durationSeconds,
        String kind,
        double tempBandScale,
        double qiCostScale,
        String smokeColor
    ) {
        public double remainingFraction() {
            return durationSeconds <= 0 ? 0.0 : Math.max(0, Math.min(1, remainingMillis / (durationSeconds * 1000.0)));
        }

        public static Snapshot empty() {
            return new Snapshot(State.EMPTY, 0L, 0, "", 1.0, 1.0, "#9DAF9E");
        }
    }
}
