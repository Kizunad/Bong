package com.bong.client.insight;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * 把玩家在 InsightOfferScreen 做出的决定回传给服务端。
 *
 * <p>真正的网络通道由 {@code BongNetworkHandler} 在后续接入；当前默认实现仅打日志，
 * 供 UI 单独验证。
 */
public interface InsightChoiceDispatcher {
    void dispatch(InsightDecision decision);

    /** 没接入网络时使用，仅打印日志。 */
    InsightChoiceDispatcher LOGGING = new InsightChoiceDispatcher() {
        private final Logger log = LoggerFactory.getLogger("bong-client.insight");

        @Override
        public void dispatch(InsightDecision decision) {
            log.info("[insight] {} -> {}", decision.triggerId(), decision.summary());
        }
    };
}
