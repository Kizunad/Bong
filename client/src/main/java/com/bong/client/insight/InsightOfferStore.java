package com.bong.client.insight;

import java.util.Objects;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;

/**
 * 当前待决的顿悟邀约 (单 slot——同一时刻最多一个 offer 在 UI 中可见)。
 *
 * <p>跟其他 store 一样：{@code volatile} 快照 + 监听器列表；派发器单独注入，便于测试用 mock。
 *
 * <p><b>offer 实例身份</b>（r7-insight-settlement.tsv identity_rule）：每个 offer 实例由
 * 不可复用的 {@link SessionToken} 标识，与 viewModel 的 {@code offerId} 一一对应。同一
 * {@code triggerId} 的先后两份 offer（wire {@code offer_id} 不同）是不同实例，各自独立
 * 结算；旧实例的任何迟到 terminal 回调（timeout/close/removal/decision/duplicate）都只能
 * compare-and-clear 自己的实例，不能清除后来 current/pending 的 offer B。
 *
 * <p>替换语义（REPLACED_BY_DIFFERENT_OFFER）：新 offer 推入时，若 current 与 pending 都已
 * 有主，新 offer 进入 bounded pending 槽等重试；否则先对 outgoing offerId 提交本地 terminal
 * tombstone（无 wire decline——当前 C2S {@code InsightDecision} 只有 trigger_id/choice_idx），
 * 再原子替换 current 实例；settlement claim 先于任何 dispatch/transition 副作用。
 */
public final class InsightOfferStore {
    private static final int MAX_PENDING_OFFERS = 1;

    private static final AtomicReference<ActiveOffer> current = new AtomicReference<>();
    private static final AtomicReference<ActiveOffer> pending = new AtomicReference<>();
    private static volatile InsightChoiceDispatcher dispatcher = InsightChoiceDispatcher.LOGGING;
    private static final CopyOnWriteArrayList<Consumer<InsightOfferViewModel>> listeners = new CopyOnWriteArrayList<>();

    private InsightOfferStore() {
    }

    public static InsightOfferViewModel snapshot() {
        ActiveOffer active = current.get();
        return active == null ? null : active.viewModel();
    }

    public static InsightOfferViewModel pendingOffer() {
        ActiveOffer active = pending.get();
        return active == null ? null : active.viewModel();
    }

    /**
     * 推送新邀约 (null = 当前 offer 已结算 / 取消)。新 offer 实例总是取得新 session token；
     * 若当前槽已有主，先把 outgoing offerId 结算为本地终态 tombstone，再原子替换。
     */
    public static void replace(InsightOfferViewModel next) {
        if (next == null) {
            clearCurrent(null);
            return;
        }
        ActiveOffer installed = new ActiveOffer(new SessionToken(), next);
        while (true) {
            ActiveOffer currentOffer = current.get();
            if (currentOffer != null) {
                if (!settleOutgoingTerminal(currentOffer)) {
                    // outgoing 已在他处结算（e.g. 玩家恰在此时提交）：不重放，直接重试安装
                    if (current.compareAndSet(currentOffer, null)) {
                        notifyListeners(null);
                    }
                    continue;
                }
            }
            if (current.compareAndSet(currentOffer, installed)) {
                break;
            }
        }
        notifyListeners(installed.viewModel());
    }

    /**
     * 结算 outgoing offer：已在 pending 槽的实例直接释放；当前槽实例只释放且**不发送**
     * wire decline（当前 C2S schema 无 offerId 关联，见 r7-insight-settlement.tsv
     * REPLACED_BY_DIFFERENT_OFFER commit_order）。返回 true 表示该实例此前仍是权威待决，
     * 由本次调用完成终态；false 表示已在他处结算（幂等 no-op，不得再次 dispatch）。
     */
    private static boolean settleOutgoingTerminal(ActiveOffer outgoing) {
        while (true) {
            ActiveOffer p = pending.get();
            if (p != null && p.token() == outgoing.token()) {
                if (pending.compareAndSet(p, null)) {
                    return true;
                }
                continue;
            }
            break;
        }
        return false;
    }

    /**
     * 玩家做出决定——先对 exact offerId 原子 claim（compare-and-clear 只作用于 matching
     * current 实例），claim 成功才 dispatch 并清空当前 slot；claim 失败 (stale/duplicate) 为
     * 幂等 no-op，绝不触碰后来 offer 的 current/pending 实例。
     */
    public static void submit(InsightDecision decision, String offerId) {
        if (offerId == null || offerId.isBlank()) {
            return;
        }
        while (true) {
            ActiveOffer active = current.get();
            if (active == null) {
                return;
            }
            if (!active.viewModel().offerId().equals(offerId)) {
                return;
            }
            if (current.compareAndSet(active, null)) {
                dispatcher.dispatch(decision, active.viewModel());
                notifyListeners(null);
                return;
            }
        }
    }

    /** 与 {@link #submit(InsightDecision, String)} 等价的便捷入口。 */
    public static void submit(InsightDecision decision, InsightOfferViewModel offer) {
        submit(decision, offer == null ? null : offer.offerId());
    }

    /** 仅当快照仍是该 offer 实例时替换（同实例刷新保留 token）。 */
    public static void replaceIfCurrent(InsightOfferViewModel next) {
        Objects.requireNonNull(next, "next");
        while (true) {
            ActiveOffer active = current.get();
            if (active != null && active.viewModel().offerId().equals(next.offerId())) {
                if (current.compareAndSet(active, new ActiveOffer(active.token(), next))) {
                    notifyListeners(next);
                    return;
                }
                continue;
            }
            return;
        }
    }

    /**
     * 精确结算当前槽中该 offer 实例（compare-and-clear）。claim 成功后以 {@code decision}
     * 发送终态并清空当前 slot；不匹配或已空为幂等 no-op。
     */
    public static void settleIfCurrent(String offerId, InsightDecision decision) {
        if (offerId == null || offerId.isBlank() || decision == null) {
            return;
        }
        while (true) {
            ActiveOffer active = current.get();
            if (active == null) {
                return;
            }
            if (!active.viewModel().offerId().equals(offerId)) {
                return;
            }
            if (current.compareAndSet(active, null)) {
                dispatcher.dispatch(decision, active.viewModel());
                notifyListeners(null);
                return;
            }
        }
    }

    /**
     * 无条件清空当前槽（不发送任何 wire 终态）。断线清理、服务端撤回等无玩家语义的
     * 场景使用；有玩家语义的结算必须走 {@link #settleIfCurrent}。
     */
    public static void clearCurrent(InsightOfferViewModel expected) {
        while (true) {
            ActiveOffer active = current.get();
            if (active == null) {
                return;
            }
            if (expected != null && active.viewModel() != expected) {
                return;
            }
            if (current.compareAndSet(active, null)) {
                notifyListeners(null);
                return;
            }
        }
    }

    /** 当前槽实例的 session token；viewModel 不在 current 槽（已被替换）时返回 null。 */
    static SessionToken sessionTokenFor(InsightOfferViewModel expected) {
        ActiveOffer active = current.get();
        return active != null && active.viewModel() == expected ? active.token() : null;
    }

    /** 该 token 是否仍拥有 current 槽（同一 offer 实例的延续刷新保留 token）。 */
    static boolean isCurrent(SessionToken expected) {
        ActiveOffer active = current.get();
        return active != null && active.token() == expected;
    }

    /** 监听器收到的是 current 槽 viewModel（null = 当前 offer 已结算 / 取消）。 */
    public static void addListener(Consumer<InsightOfferViewModel> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<InsightOfferViewModel> listener) {
        listeners.remove(listener);
    }

    public static void setDispatcher(InsightChoiceDispatcher next) {
        dispatcher = next == null ? InsightChoiceDispatcher.LOGGING : next;
    }

    public static InsightChoiceDispatcher dispatcher() {
        return dispatcher;
    }

    /**
     * 断线时调用：仅清 current/pending 快照，保留 dispatcher 与 listeners。
     *
     * <p>之前此处误用 {@link #resetForTests()} —— 它会一并拆掉监听器和真实
     * dispatcher，导致重连后 offer 不再开屏、玩家选择也不再回传服务端。
     */
    public static void clearOnDisconnect() {
        clearCurrent(null);
        pending.set(null);
    }

    public static void resetForTests() {
        current.set(null);
        pending.set(null);
        dispatcher = InsightChoiceDispatcher.LOGGING;
        listeners.clear();
    }

    private static void notifyListeners(InsightOfferViewModel next) {
        for (Consumer<InsightOfferViewModel> listener : listeners) {
            listener.accept(next);
        }
    }

    /** offer 实例会话令牌：与 ScrollReadStore.SessionToken 同构，防 ABA 复用。 */
    static final class SessionToken {
        private SessionToken() {
        }
    }

    static record ActiveOffer(SessionToken token, InsightOfferViewModel viewModel) {
    }
}
