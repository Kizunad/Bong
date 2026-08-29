package com.bong.client.craft;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;

import java.util.Objects;

/** 把手搓 typed intent 适配到既有 C2S sender，不解释服务端业务结果。 */
public final class CraftClientIntentSink implements UiIntentSink<CraftIntent> {
    private final Transport transport;

    CraftClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    public static CraftClientIntentSink production() {
        return new CraftClientIntentSink(new Transport() {
            @Override
            public void start(String recipeId, int quantity) {
                ClientRequestSender.sendCraftStart(recipeId, quantity);
            }

            @Override
            public void cancel() {
                ClientRequestSender.sendCraftCancel();
            }
        });
    }

    @Override
    public UiIntentResult dispatch(CraftIntent intent) {
        if (intent == null) {
            return UiIntentResult.rejected("craft intent must not be null");
        }
        try {
            if (intent instanceof CraftIntent.Start start) {
                String recipeId = normalize(start.recipeId());
                if (recipeId == null) {
                    return UiIntentResult.rejected("recipe id must not be blank");
                }
                if (start.quantity() < 1) {
                    return UiIntentResult.rejected("quantity must be >= 1");
                }
                transport.start(recipeId, start.quantity());
                return UiIntentResult.accepted();
            }
            transport.cancel();
            return UiIntentResult.accepted();
        } catch (RuntimeException failure) {
            String detail = normalize(failure.getMessage());
            return UiIntentResult.error(
                "craft transport failed: " + (detail == null ? failure.getClass().getSimpleName() : detail)
            );
        }
    }

    private static String normalize(String value) {
        if (value == null || value.isBlank()) {
            return null;
        }
        return value.strip();
    }

    interface Transport {
        void start(String recipeId, int quantity);

        void cancel();
    }
}
