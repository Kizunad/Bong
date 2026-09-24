package com.bong.client.combat;

import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;

import java.util.Objects;

/** 将阵法布置意图适配到既有 C2S sender；XML Screen 不直接依赖网络设施。 */
public final class ZhenfaLayoutClientIntentSink implements UiIntentSink<ZhenfaLayoutIntent> {
    private final Transport transport;

    ZhenfaLayoutClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    /** 生产组装只由应用组合根调用，保留既有 zhenfa_place wire 形状。 */
    public static ZhenfaLayoutClientIntentSink production() {
        return new ZhenfaLayoutClientIntentSink(intent -> ClientRequestSender.sendZhenfaPlace(
            new net.minecraft.util.math.BlockPos(intent.x(), intent.y(), intent.z()),
            parseKind(intent.kind()), parseCarrier(intent.carrier()), intent.qiInvestRatio(),
            intent.trigger(), intent.itemInstanceId(), parseFace(intent.targetFace())));
    }

    @Override
    public UiIntentResult dispatch(ZhenfaLayoutIntent intent) {
        if (intent == null) return UiIntentResult.rejected("zhenfa intent must not be null");
        try {
            if (!(intent instanceof ZhenfaLayoutIntent.Place place)) {
                return UiIntentResult.rejected("unsupported zhenfa intent");
            }
            transport.place(place);
            return UiIntentResult.accepted();
        } catch (IllegalArgumentException failure) {
            return UiIntentResult.rejected(failure.getMessage());
        } catch (RuntimeException failure) {
            String detail = failure.getMessage();
            return UiIntentResult.error("zhenfa transport failed: "
                + (detail == null || detail.isBlank() ? failure.getClass().getSimpleName() : detail));
        }
    }

    private static ClientRequestProtocol.ZhenfaKind parseKind(String kind) {
        for (ClientRequestProtocol.ZhenfaKind value : ClientRequestProtocol.ZhenfaKind.values()) {
            if (value.wireName().equals(kind)) return value;
        }
        throw new IllegalArgumentException("unknown zhenfa kind: " + kind);
    }

    private static ClientRequestProtocol.ZhenfaCarrierKind parseCarrier(String carrier) {
        if (carrier == null) return null;
        for (ClientRequestProtocol.ZhenfaCarrierKind value : ClientRequestProtocol.ZhenfaCarrierKind.values()) {
            if (value.wireName().equals(carrier)) return value;
        }
        throw new IllegalArgumentException("unknown zhenfa carrier: " + carrier);
    }

    private static ClientRequestProtocol.ZhenfaTargetFace parseFace(String face) {
        if (face == null) return null;
        for (ClientRequestProtocol.ZhenfaTargetFace value : ClientRequestProtocol.ZhenfaTargetFace.values()) {
            if (value.wireName().equals(face)) return value;
        }
        throw new IllegalArgumentException("unknown zhenfa target face: " + face);
    }

    @FunctionalInterface
    interface Transport {
        void place(ZhenfaLayoutIntent.Place intent);
    }
}
