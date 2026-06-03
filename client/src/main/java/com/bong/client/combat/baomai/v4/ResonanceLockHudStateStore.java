package com.bong.client.combat.baomai.v4;

import java.util.concurrent.atomic.AtomicReference;

/**
 * plan-combat-skill-feedback-bridges-v1 P1 — 共振锁定 HUD 状态存储。
 *
 * 存储当前共振锁定状态，供 {@link ResonanceLockMeterHud} 渲染进度弧形 meter。
 * 接收 {@code bong:resonance_lock}（locked=true）和
 * {@code bong:resonance_lock_end}（locked=false）两个 channel 的更新。
 */
public final class ResonanceLockHudStateStore {

    private static final AtomicReference<State> CURRENT = new AtomicReference<>(State.UNLOCKED);

    private ResonanceLockHudStateStore() {}

    /**
     * 收到 {@code bong:resonance_lock} 后调用。
     *
     * @param partnerId   对手 entity wire id
     * @param startedAtTick 锁定起始 tick
     * @param endsAtTick   锁定结束 tick
     */
    public static void onLockStarted(String partnerId, long startedAtTick, long endsAtTick) {
        CURRENT.set(new State(true, partnerId, startedAtTick, endsAtTick));
    }

    /**
     * 收到 {@code bong:resonance_lock_end} 后调用。
     */
    public static void onLockEnded() {
        CURRENT.set(State.UNLOCKED);
    }

    /** 当前快照。 */
    public static State snapshot() {
        return CURRENT.get();
    }

    public static void clear() {
        CURRENT.set(State.UNLOCKED);
    }

    // ── nested types ──────────────────────────────────────────────────────────

    public static final class State {
        public static final State UNLOCKED = new State(false, "", 0L, 0L);

        /** true = 当前处于共振锁定中 */
        public final boolean locked;
        /** 对手 entity wire id */
        public final String partnerId;
        /** 锁定起始 tick（用于进度计算） */
        public final long startedAtTick;
        /** 锁定结束 tick */
        public final long endsAtTick;

        private State(boolean locked, String partnerId, long startedAtTick, long endsAtTick) {
            this.locked = locked;
            this.partnerId = partnerId;
            this.startedAtTick = startedAtTick;
            this.endsAtTick = endsAtTick;
        }

        /**
         * 0.0（刚锁定）→ 1.0（即将到期）。
         *
         * @param currentTick 当前服务端 tick（可用客户端估算值）
         */
        public double progress(long currentTick) {
            long total = Math.max(1L, endsAtTick - startedAtTick);
            long elapsed = Math.max(0L, currentTick - startedAtTick);
            return Math.min(1.0, (double) elapsed / (double) total);
        }

        /**
         * 剩余 tick 数（0 表示已到期）。
         */
        public long remainingTicks(long currentTick) {
            return Math.max(0L, endsAtTick - currentTick);
        }
    }
}
