package com.bong.client.insight;

import com.bong.client.network.ClientRequestSender;

import java.util.List;
import java.util.Objects;
import java.util.function.BiConsumer;
import java.util.function.Consumer;
import java.util.function.Supplier;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * 真正的 {@link InsightChoiceDispatcher} 实现：把玩家决定编码为
 * {@code client_request} 载荷并通过 {@link ClientRequestSender} 发往服务端。
 *
 * <p>服务端仅理解「候选下标 idx」而非 client 侧的 {@code choiceId} 字符串，
 * 所以本类从当前 {@link InsightOfferStore} 快照里把 {@code choiceId → idx} 解析出来。
 * 若 idx 解析失败（offer 已被置换 / id 不匹配），降级为 DECLINED。
 * {@code heart_demon:*} trigger 复用同一 UI，但回传 {@code heart_demon_decision}。
 */
public final class ClientRequestInsightDispatcher implements InsightChoiceDispatcher {

    private static final Logger LOG = LoggerFactory.getLogger("bong-client.insight");
    private static final String HEART_DEMON_TRIGGER_PREFIX = "heart_demon:";

    private final Supplier<InsightOfferViewModel> offerSupplier;
    private final BiConsumer<String, Integer> sendFn;
    private final Consumer<Integer> heartDemonSendFn;

    public ClientRequestInsightDispatcher() {
        this(
            InsightOfferStore::snapshot,
            ClientRequestSender::sendInsightDecision,
            ClientRequestSender::sendHeartDemonDecision
        );
    }

    ClientRequestInsightDispatcher(Supplier<InsightOfferViewModel> offerSupplier,
                                    BiConsumer<String, Integer> sendFn) {
        this(offerSupplier, sendFn, ignored -> {});
    }

    ClientRequestInsightDispatcher(Supplier<InsightOfferViewModel> offerSupplier,
                                   BiConsumer<String, Integer> sendFn,
                                   Consumer<Integer> heartDemonSendFn) {
        this.offerSupplier = Objects.requireNonNull(offerSupplier, "offerSupplier");
        this.sendFn = Objects.requireNonNull(sendFn, "sendFn");
        this.heartDemonSendFn = Objects.requireNonNull(heartDemonSendFn, "heartDemonSendFn");
    }

    @Override
    public void dispatch(InsightDecision decision) {
        Objects.requireNonNull(decision, "decision");
        Integer idx = resolveIdx(decision);
        LOG.info("[insight] dispatch {} -> {} (idx={})", decision.triggerId(), decision.summary(), idx);
        if (decision.triggerId().startsWith(HEART_DEMON_TRIGGER_PREFIX)) {
            heartDemonSendFn.accept(idx);
            return;
        }
        sendFn.accept(decision.triggerId(), idx);
    }

    private Integer resolveIdx(InsightDecision decision) {
        if (decision.kind() != InsightDecision.Kind.CHOSEN) {
            return null;
        }
        InsightOfferViewModel offer = offerSupplier.get();
        if (offer == null || !offer.triggerId().equals(decision.triggerId())) {
            LOG.warn("[insight] cannot resolve idx: offer snapshot missing or stale for {}", decision.triggerId());
            return null;
        }
        List<InsightChoice> choices = offer.choices();
        for (int i = 0; i < choices.size(); i++) {
            if (choices.get(i).choiceId().equals(decision.chosenChoiceId())) {
                return i;
            }
        }
        LOG.warn("[insight] choiceId {} not found in offer {}; downgrading to DECLINED",
            decision.chosenChoiceId(), decision.triggerId());
        return null;
    }
}
