package com.bong.client.combat;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import com.google.gson.JsonObject;

import java.util.Objects;

/** 将暗器注入意图适配到既有 C2S sender；Screen 不直接依赖网络设施。 */
public final class ForgeCarrierClientIntentSink implements UiIntentSink<ForgeCarrierIntent> {
    private final Transport transport;

    ForgeCarrierClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    /** 生产组装只由应用组合根调用，保留既有 request type 和 payload 字段。 */
    public static ForgeCarrierClientIntentSink production() {
        return new ForgeCarrierClientIntentSink((item, qiInvest) -> {
            JsonObject payload = new JsonObject();
            payload.addProperty("item", item);
            payload.addProperty("qi_invest", qiInvest);
            ClientRequestSender.send("combat.forge_carrier_begin", payload);
        });
    }

    @Override
    public UiIntentResult dispatch(ForgeCarrierIntent intent) {
        if (intent == null) {
            return UiIntentResult.rejected("forge carrier intent must not be null");
        }
        try {
            if (intent instanceof ForgeCarrierIntent.Begin begin) {
                transport.begin(begin.item(), begin.qiInvest());
                return UiIntentResult.accepted();
            }
            throw new IllegalStateException(
                "unsupported forge carrier intent: " + intent.getClass().getName());
        } catch (RuntimeException failure) {
            String detail = failure.getMessage();
            return UiIntentResult.error("forge carrier transport failed: "
                + (detail == null || detail.isBlank()
                    ? failure.getClass().getSimpleName() : detail));
        }
    }

    @FunctionalInterface
    interface Transport {
        void begin(String item, double qiInvest);
    }
}
