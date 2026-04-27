package com.bong.client.hud;

/**
 * plan-tsy-container-v1 §5.2 — TSY 容器搜刮 HUD 客户端状态快照。
 *
 * <p>纯数据 record；由后续 client IPC bridge（接 SearchStartedV1 /
 * SearchProgressV1 / SearchCompletedV1 / SearchAbortedV1 payload）维护。
 *
 * <p>状态机：
 * <pre>
 *   IDLE → SEARCHING（收 SearchStartedV1）
 *   SEARCHING → SEARCHING（收 SearchProgressV1，更新 elapsed）
 *   SEARCHING → COMPLETED_FLASH（收 SearchCompletedV1，3 秒后回 IDLE）
 *   SEARCHING → ABORTED_FLASH（收 SearchAbortedV1，1 秒后回 IDLE）
 * </pre>
 */
public record SearchHudState(
    Phase phase,
    String containerKindZh,
    int requiredTicks,
    int elapsedTicks,
    AbortReason abortReason
) {
    public enum Phase {
        IDLE,
        SEARCHING,
        COMPLETED_FLASH,
        ABORTED_FLASH,
    }

    public enum AbortReason {
        NONE,
        MOVED,
        COMBAT,
        DAMAGED,
        CANCELLED,
    }

    public static SearchHudState idle() {
        return new SearchHudState(Phase.IDLE, "", 0, 0, AbortReason.NONE);
    }

    public static SearchHudState searching(String containerKindZh, int requiredTicks, int elapsedTicks) {
        return new SearchHudState(Phase.SEARCHING, containerKindZh, requiredTicks, elapsedTicks, AbortReason.NONE);
    }

    public static SearchHudState completed(String containerKindZh) {
        return new SearchHudState(Phase.COMPLETED_FLASH, containerKindZh, 0, 0, AbortReason.NONE);
    }

    public static SearchHudState aborted(String containerKindZh, AbortReason reason) {
        return new SearchHudState(Phase.ABORTED_FLASH, containerKindZh, 0, 0, reason);
    }

    /** 进度比 [0.0, 1.0]；非 SEARCHING 阶段返回 0。 */
    public float progressRatio() {
        if (phase != Phase.SEARCHING || requiredTicks <= 0) {
            return 0f;
        }
        return Math.min(1f, (float) elapsedTicks / (float) requiredTicks);
    }

    /** 剩余秒（向上取整，20 tps）；非 SEARCHING 阶段返回 0。 */
    public int remainingSeconds() {
        if (phase != Phase.SEARCHING || requiredTicks <= 0) {
            return 0;
        }
        int remaining = Math.max(0, requiredTicks - elapsedTicks);
        return (remaining + 19) / 20;
    }
}
