package com.bong.client.combat;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;

import java.util.Objects;

/** 将死亡屏动作适配到既有 C2S sender；Screen 不直接依赖网络设施。 */
public final class DeathClientIntentSink implements UiIntentSink<DeathIntent> {
    private final Transport transport;

    DeathClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    /** 生产组装只由应用层调用，保留既有 request type 和 wire 语义。 */
    public static DeathClientIntentSink production() {
        return new DeathClientIntentSink(new Transport() {
            @Override
            public void reincarnate() {
                ClientRequestSender.send("combat_reincarnate", null);
            }

            @Override
            public void terminate() {
                ClientRequestSender.send("combat_terminate", null);
            }
        });
    }

    @Override
    public UiIntentResult dispatch(DeathIntent intent) {
        if (intent == null) {
            return UiIntentResult.rejected("death intent must not be null");
        }
        try {
            if (intent instanceof DeathIntent.Reincarnate) {
                transport.reincarnate();
                return UiIntentResult.accepted();
            }
            if (intent instanceof DeathIntent.Terminate) {
                transport.terminate();
                return UiIntentResult.accepted();
            }
            throw new IllegalStateException("unsupported death intent: " + intent.getClass().getName());
        } catch (RuntimeException failure) {
            String detail = failure.getMessage();
            return UiIntentResult.error("death transport failed: "
                + (detail == null || detail.isBlank() ? failure.getClass().getSimpleName() : detail));
        }
    }

    interface Transport {
        void reincarnate();

        void terminate();
    }
}
