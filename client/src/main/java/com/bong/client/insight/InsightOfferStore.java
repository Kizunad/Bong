package com.bong.client.insight;

import java.util.HashMap;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * 当前待决的顿悟邀约 (单 slot——同一时刻最多一个 offer 在 UI 中可见)。
 *
 * <p>跟其他 store 一样：{@code volatile} 快照 + 监听器列表。
 * 派发器单独注入，便于测试用 mock。
 */
public final class InsightOfferStore {
    private static volatile InsightOfferViewModel snapshot = null;
    private static volatile InsightChoiceDispatcher dispatcher = InsightChoiceDispatcher.LOGGING;
    private static final Map<String, TerminalCause> terminalCauses = new HashMap<>();
    private static final CopyOnWriteArrayList<Consumer<InsightOfferViewModel>> listeners = new CopyOnWriteArrayList<>();

    private InsightOfferStore() {
    }

    public static InsightOfferViewModel snapshot() {
        return snapshot;
    }

    /** 推送新邀约 (null = 当前 offer 已结算 / 取消)。 */
    public static void replace(InsightOfferViewModel next) {
        snapshot = next;
        for (Consumer<InsightOfferViewModel> listener : listeners) {
            listener.accept(next);
        }
    }

    public static void addListener(Consumer<InsightOfferViewModel> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<InsightOfferViewModel> listener) {
        listeners.remove(listener);
    }

    /** 玩家做出决定——回传服务端并清空当前 slot。 */
    public static void submit(InsightDecision decision) {
        InsightOfferViewModel current = snapshot;
        if (current != null && current.triggerId().equals(decision.triggerId())) {
            settle(current, terminalCause(decision), decision);
            return;
        }
        dispatcher.dispatch(decision);
    }

    private static TerminalCause terminalCause(InsightDecision decision) {
        return switch (decision.kind()) {
            case CHOSEN -> TerminalCause.ACCEPT;
            case DECLINED -> TerminalCause.DECLINE;
            case TIMED_OUT -> TerminalCause.TIMEOUT;
        };
    }

    public static synchronized boolean settle(
        InsightOfferViewModel offer,
        TerminalCause cause,
        InsightDecision decision
    ) {
        Objects.requireNonNull(offer, "offer");
        Objects.requireNonNull(cause, "cause");
        Objects.requireNonNull(decision, "decision");
        if (!offer.triggerId().equals(decision.triggerId()) || terminalCauses.containsKey(offer.triggerId())) {
            return false;
        }
        terminalCauses.put(offer.triggerId(), cause);
        try {
            dispatcher.dispatch(decision);
        } finally {
            InsightOfferViewModel current = snapshot;
            if (current != null && current.triggerId().equals(offer.triggerId())) {
                replace(null);
            }
        }
        return true;
    }

    static synchronized TerminalCause terminalCauseForTests(String triggerId) {
        return terminalCauses.get(triggerId);
    }

    public enum TerminalCause {
        ACCEPT,
        DECLINE,
        TIMEOUT,
        ESC,
        REPLACED_BY_DIFFERENT_OFFER,
        REMOVED_EXCEPTIONALLY
    }

    public static void setDispatcher(InsightChoiceDispatcher next) {
        dispatcher = next == null ? InsightChoiceDispatcher.LOGGING : next;
    }

    public static InsightChoiceDispatcher dispatcher() {
        return dispatcher;
    }

    /**
     * 断线时调用：仅清当前 offer 快照，保留 dispatcher 与 listeners。
     *
     * <p>之前此处误用 {@link #resetForTests()} —— 它会一并拆掉监听器和真实
     * dispatcher，导致重连后 offer 不再开屏、玩家选择也不再回传服务端。
     */
    public static void clearOnDisconnect() {
        replace(null);
    }

    public static void resetForTests() {
        snapshot = null;
        dispatcher = InsightChoiceDispatcher.LOGGING;
        synchronized (InsightOfferStore.class) {
            terminalCauses.clear();
        }
        listeners.clear();
    }
}
