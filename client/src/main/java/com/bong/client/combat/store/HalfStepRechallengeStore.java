package com.bong.client.combat.store;

/**
 * 半步化虚重渡触发 HUD 状态（plan-halfstep-rechallenge-integration-v1 P0）。
 * Last-write-wins；单玩家单条记录。
 *
 * <p>三终止条件：
 * <ol>
 *   <li>服务器显式 {@code active=false} HIDE payload → {@link #clear()} 立即清场。</li>
 *   <li>玩家化虚 settle → 服务器再发一次 {@code active=false}（由 emit 系统负责）。</li>
 *   <li>过窗（{@code currentTick > windowUntilTick}）→ client 本地在 Planner 中判定自动淡出。</li>
 * </ol>
 */
public final class HalfStepRechallengeStore {

    public record State(
        boolean active,
        String charId,
        long windowUntilTick,
        long atTick,
        /** 本地触发时的墙钟毫秒（用于淡入计时）。 */
        long triggeredAtMs
    ) {
        public State {
            charId = charId == null ? "" : charId;
        }

        public static final State NONE = new State(false, "", 0L, 0L, 0L);

        /**
         * 以实时毫秒估算剩余窗口毫秒。
         * 公式：windowUntilTick - atTick 为剩余 ticks，× 50ms/tick = 剩余 ms，
         * 再减去自触发以来流逝的实时时间。
         */
        public long remainingMs(long nowMs) {
            long ticksRemaining = windowUntilTick - atTick;
            long totalWindowMs = ticksRemaining * 50L;
            long elapsed = Math.max(0L, nowMs - triggeredAtMs);
            return Math.max(0L, totalWindowMs - elapsed);
        }

        /**
         * 窗口是否已过（以实时毫秒为基准）。
         * Planner 用此方法决定是否本地淡出。
         */
        public boolean windowExpired(long nowMs) {
            return active && triggeredAtMs > 0L && remainingMs(nowMs) <= 0L;
        }
    }

    private static volatile State snapshot = State.NONE;

    private HalfStepRechallengeStore() {}

    public static State snapshot() {
        return snapshot;
    }

    /** 更新为最新状态（last-write-wins）。 */
    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    /** 显式清场（active=false payload 触发）。 */
    public static void clear() {
        snapshot = State.NONE;
    }

    /** 测试用重置。 */
    public static void resetForTests() {
        snapshot = State.NONE;
    }
}
