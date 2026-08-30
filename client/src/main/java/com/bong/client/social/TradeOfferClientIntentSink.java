package com.bong.client.social;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;

import java.util.Objects;

/** 把交易 response 映射到既有 C2S sender；不读取 Store，也不声称服务端已接受。 */
public final class TradeOfferClientIntentSink implements UiIntentSink<TradeOfferIntent> {
    private final Transport transport;

    TradeOfferClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    public static TradeOfferClientIntentSink production() {
        return new TradeOfferClientIntentSink(new Transport() {
            @Override
            public void respond(String offerId, boolean accepted, Long requestedInstanceId) {
                ClientRequestSender.sendTradeOfferResponse(offerId, accepted, requestedInstanceId);
            }

            @Override
            public void request(String target, long offeredInstanceId) {
                ClientRequestSender.sendTradeOfferRequest(target, offeredInstanceId);
            }
        });
    }

    @Override
    public UiIntentResult dispatch(TradeOfferIntent intent) {
        if (intent == null) return UiIntentResult.rejected("trade intent must not be null");
        if (intent instanceof TradeOfferIntent.Request request) {
            if (request.target() == null || request.target().isBlank()) {
                return UiIntentResult.rejected("trade target must not be blank");
            }
            if (request.offeredInstanceId() <= 0L) {
                return UiIntentResult.rejected("offered item requires an exact instance_id");
            }
            try {
                transport.request(request.target().strip(), request.offeredInstanceId());
                return UiIntentResult.accepted();
            } catch (RuntimeException failure) {
                return transportError(failure);
            }
        }
        if (!(intent instanceof TradeOfferIntent.Respond response)) {
            return UiIntentResult.rejected("unsupported trade intent");
        }
        if (response.offerId() == null || response.offerId().isBlank()) {
            return UiIntentResult.rejected("offer id must not be blank");
        }
        if (response.accepted() && (response.requestedInstanceId() == null || response.requestedInstanceId() <= 0L)) {
            return UiIntentResult.rejected("accepted trade requires an exact item instance_id");
        }
        if (!response.accepted() && response.requestedInstanceId() != null) {
            return UiIntentResult.rejected("declined trade must not carry an item instance_id");
        }
        try {
            String offerId = response.offerId().strip();
            transport.respond(offerId, response.accepted(), response.requestedInstanceId());
            // 本地 offer 只在 transport 成功后清理；失败时保留，允许上层重试。
            SocialStateStore.clearTradeOffer(offerId);
            return UiIntentResult.accepted();
        } catch (RuntimeException failure) {
            return transportError(failure);
        }
    }

    private static UiIntentResult transportError(RuntimeException failure) {
        String detail = failure.getMessage();
        return UiIntentResult.error("trade transport failed: "
            + (detail == null || detail.isBlank() ? failure.getClass().getSimpleName() : detail));
    }

    interface Transport {
        void respond(String offerId, boolean accepted, Long requestedInstanceId);

        default void request(String target, long offeredInstanceId) {
            throw new UnsupportedOperationException("trade request transport is not configured");
        }
    }
}
