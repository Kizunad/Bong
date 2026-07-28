package com.bong.client.combat.baomai.v4;

import java.util.Collections;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;

/**
 * plan-combat-skill-feedback-bridges-v1 P1 — 裂读 HUD 状态存储。
 *
 * 存储最近一次 {@code bong:crack_reading} 收到的裂读结果，
 * 供 {@link CrackReadingOverlay} 渲染经脉 bracket overlay。
 * 超过 {@code displayTicks * 50ms} 后数据自动过期。
 */
public final class CrackReadingHudStateStore {

    private static final AtomicReference<State> CURRENT = new AtomicReference<>(State.EMPTY);

    private CrackReadingHudStateStore() {}

    /** 解析 JSON 后写入新状态。 */
    public static void accept(
            long targetEntityId,
            List<MeridianEntry> entries,
            boolean isDeep,
            long displayTicks
    ) {
        long expiresAtMs = System.currentTimeMillis() + Math.max(1L, displayTicks) * 50L;
        CURRENT.set(new State(targetEntityId, List.copyOf(entries), isDeep, expiresAtMs));
    }

    /** 当前有效快照（超期返回 EMPTY）。 */
    public static State snapshot() {
        return snapshot(System.currentTimeMillis());
    }

    /** testable overload */
    static State snapshot(long nowMs) {
        State s = CURRENT.get();
        return (s != State.EMPTY && nowMs >= s.expiresAtMs) ? State.EMPTY : s;
    }

    public static void clear() {
        CURRENT.set(State.EMPTY);
    }

    // ── nested types ──────────────────────────────────────────────────────────

    public record MeridianEntry(
            String meridian,
            String bracket,
            boolean hasCircuit,
            boolean isDeadArmor
    ) {}

    public static final class State {
        public static final State EMPTY = new State(-1L, List.of(), false, 0L);

        public final long targetEntityId;
        public final List<MeridianEntry> entries;
        public final boolean isDeep;
        public final long expiresAtMs;

        private State(long targetEntityId, List<MeridianEntry> entries, boolean isDeep, long expiresAtMs) {
            this.targetEntityId = targetEntityId;
            this.entries = entries;
            this.isDeep = isDeep;
            this.expiresAtMs = expiresAtMs;
        }

        public boolean isActive() {
            return this != EMPTY;
        }
    }
}
