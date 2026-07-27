package com.bong.client.combat;

import com.bong.client.BongClient;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * Volatile cast-state store (§11.1). Authoritative data comes from the server
 * via {@code bong:combat/cast_sync}; this mirror is read by the HUD planner and
 * by local predicted-interrupt checks.
 */
public final class CastStateStore {
    /** Default short cooldown after an interrupt (§4.4). */
    public static final long SHORT_INTERRUPT_COOLDOWN_MS = 500L;

    /**
     * 状态转换的<b>来源</b>——决定该转换是否可作为「服务端已 accepted」的凭据。
     *
     * <p>本 store 是**混合**镜像：既落地服务端权威 {@code cast_sync} 回执，也承载纯客户端的
     * 乐观预测（{@link #beginSkillBarCast} 在按键那一刻就写 CASTING，服务端还没回话；
     * {@link #tick} 到点就本地 {@code transitionToComplete}）。只看快照分不出二者，任何
     * 「必须已被服务端确认才能做」的消费方（如 plan-fpv-cast-av-v1 §P3 的施法 juice 门控）
     * 都需要这个来源标记，否则会把预测当成确认。
     */
    public enum Origin {
        /** 纯客户端乐观预测（按键起手 / 本地计时到点 / 本地预测打断），<b>不是</b> accepted 凭据。 */
        LOCAL_PREDICTION,
        /** 服务端 {@code bong:combat/cast_sync} 回执落地（唯一入口 {@link #replace}）。 */
        SERVER_AUTHORITATIVE,
    }

    /** 带来源的转换监听（{@link #addListener} 的 {@link Consumer} 版拿不到来源）。 */
    @FunctionalInterface
    public interface TransitionListener {
        void onTransition(CastState state, Origin origin);
    }

    private static volatile CastState snapshot = CastState.idle();
    private static final List<Consumer<CastState>> listeners = new CopyOnWriteArrayList<>();
    private static final List<TransitionListener> transitionListeners = new CopyOnWriteArrayList<>();

    private CastStateStore() {
    }

    public static CastState snapshot() {
        return snapshot;
    }

    /**
     * 服务端权威回执落地（生产唯一调用方 {@code CastSyncHandler}）——转换按
     * {@link Origin#SERVER_AUTHORITATIVE} 通告。新增本方法的调用方前请确认它真的携带服务端
     * 权威信息：本方法是「accepted」语义的唯一来源，用它写本地预测会让下游门控失效。
     */
    public static void replace(CastState next) {
        setSnapshot(next == null ? CastState.idle() : next, Origin.SERVER_AUTHORITATIVE);
    }

    public static void addListener(Consumer<CastState> listener) {
        if (listener != null) listeners.add(listener);
    }

    public static void removeListener(Consumer<CastState> listener) {
        listeners.remove(listener);
    }

    public static void addTransitionListener(TransitionListener listener) {
        if (listener != null) transitionListeners.add(listener);
    }

    public static void removeTransitionListener(TransitionListener listener) {
        transitionListeners.remove(listener);
    }

    /** Begin casting (Idle → Casting). No-op if already casting. */
    public static void beginCast(int slot, int durationMs, long startedAtMs) {
        beginCast(CastState.Source.QUICK_SLOT, slot, durationMs, startedAtMs);
    }

    public static void beginSkillBarCast(int slot, int durationMs, long startedAtMs) {
        beginCast(CastState.Source.SKILL_BAR, slot, durationMs, startedAtMs);
    }

    public static void beginCast(CastState.Source source, int slot, int durationMs, long startedAtMs) {
        CastState current = snapshot;
        if (current.isCasting()) {
            return;
        }
        setSnapshot(CastState.casting(source, slot, durationMs, startedAtMs), Origin.LOCAL_PREDICTION);
    }

    /** Casting → Complete when duration has elapsed. */
    public static void complete(long nowMs) {
        CastState current = snapshot;
        if (!current.isCasting()) return;
        setSnapshot(current.transitionToComplete(nowMs), Origin.LOCAL_PREDICTION);
    }

    /** Casting → Interrupt with a reason. Idempotent (stays in interrupt state). */
    public static void interrupt(CastOutcome reason, long nowMs) {
        CastState current = snapshot;
        if (current.phase() == CastState.Phase.IDLE) return;
        if (current.phase() == CastState.Phase.INTERRUPT) return;
        setSnapshot(current.transitionToInterrupt(reason, nowMs), Origin.LOCAL_PREDICTION);
    }

    /**
     * After the 0.3s cast-bar fade (§4.1), revert to idle. Safe to call every
     * frame.
     */
    public static void tick(long nowMs) {
        CastState current = snapshot;
        if (current.phase() == CastState.Phase.CASTING) {
            if (current.durationMs() > 0
                && nowMs - current.startedAtMs() >= current.durationMs()) {
                setSnapshot(current.transitionToComplete(nowMs), Origin.LOCAL_PREDICTION);
            }
            return;
        }
        if (current.phase() == CastState.Phase.COMPLETE
            || current.phase() == CastState.Phase.INTERRUPT) {
            if (nowMs - current.endedAtMs() >= 300L) {
                setSnapshot(CastState.idle(), Origin.LOCAL_PREDICTION);
            }
        }
    }

    public static void resetForTests() {
        snapshot = CastState.idle();
        listeners.clear();
        transitionListeners.clear();
    }

    private static void setSnapshot(CastState next, Origin origin) {
        CastState current = next == null ? CastState.idle() : next;
        snapshot = current;
        for (Consumer<CastState> listener : listeners) {
            try {
                listener.accept(current);
            } catch (RuntimeException ex) {
                BongClient.LOGGER.warn("CastStateStore listener failed", ex);
            }
        }
        for (TransitionListener listener : transitionListeners) {
            try {
                listener.onTransition(current, origin);
            } catch (RuntimeException ex) {
                BongClient.LOGGER.warn("CastStateStore transition listener failed", ex);
            }
        }
    }
}
