package com.bong.client.coffin;

import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import net.minecraft.util.math.BlockPos;

import java.util.Objects;

/** 将延寿棺菜单意图适配到 C2S sender；Screen 不直接依赖网络设施。 */
public final class CoffinMenuClientIntentSink implements UiIntentSink<CoffinMenuIntent> {
    private final Transport transport;

    CoffinMenuClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    public static CoffinMenuClientIntentSink production() {
        return new CoffinMenuClientIntentSink(new Transport() {
            @Override
            public void enter(BlockPos coffinPos) {
                ClientRequestSender.sendCoffinEnter(coffinPos);
            }

            @Override
            public void reclaim(BlockPos coffinPos) {
                ClientRequestSender.sendCoffinMenuReclaim(coffinPos);
            }
        });
    }

    @Override
    public UiIntentResult dispatch(CoffinMenuIntent intent) {
        if (intent == null) {
            return UiIntentResult.rejected("coffin menu intent must not be null");
        }
        try {
            if (intent instanceof CoffinMenuIntent.Enter enter) {
                transport.enter(enter.coffinPos());
                return UiIntentResult.accepted();
            }
            if (intent instanceof CoffinMenuIntent.Reclaim reclaim) {
                transport.reclaim(reclaim.coffinPos());
                return UiIntentResult.accepted();
            }
            throw new IllegalStateException("unsupported coffin menu intent: " + intent.getClass().getName());
        } catch (RuntimeException failure) {
            String detail = failure.getMessage();
            return UiIntentResult.error("coffin menu transport failed: "
                + (detail == null || detail.isBlank() ? failure.getClass().getSimpleName() : detail));
        }
    }

    interface Transport {
        void enter(BlockPos coffinPos);

        void reclaim(BlockPos coffinPos);
    }
}
