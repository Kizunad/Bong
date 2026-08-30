package com.bong.client.combat;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;

import java.util.Objects;

/** 将终结屏 typed action 适配到 C2S sender；屏幕不直接依赖网络设施。 */
public final class TerminateClientIntentSink implements UiIntentSink<TerminateIntent> {
    private final Transport transport;

    TerminateClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    public static TerminateClientIntentSink production() {
        return new TerminateClientIntentSink(ClientRequestSender::sendCombatCreateNewCharacter);
    }

    @Override
    public UiIntentResult dispatch(TerminateIntent intent) {
        if (intent == null) {
            return UiIntentResult.rejected("terminate intent must not be null");
        }
        try {
            if (intent instanceof TerminateIntent.CreateNewCharacter) {
                transport.createNewCharacter();
                return UiIntentResult.accepted();
            }
            throw new IllegalStateException("unsupported terminate intent: " + intent.getClass().getName());
        } catch (RuntimeException failure) {
            String detail = failure.getMessage();
            return UiIntentResult.error("terminate transport failed: "
                + (detail == null || detail.isBlank() ? failure.getClass().getSimpleName() : detail));
        }
    }

    @FunctionalInterface
    interface Transport {
        void createNewCharacter();
    }
}
