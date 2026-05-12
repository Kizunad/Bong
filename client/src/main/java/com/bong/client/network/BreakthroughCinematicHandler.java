package com.bong.client.network;

import com.bong.client.cultivation.BreakthroughCinematicPayload;
import com.bong.client.cultivation.BreakthroughSpectacleRenderer;
import com.bong.client.state.SeasonStateStore;
import com.bong.client.state.VisualEffectState;

import java.util.Objects;
import java.util.function.LongSupplier;

public final class BreakthroughCinematicHandler implements ServerDataHandler {
    private final LongSupplier nowMillisSupplier;

    public BreakthroughCinematicHandler() {
        this(System::currentTimeMillis);
    }

    BreakthroughCinematicHandler(LongSupplier nowMillisSupplier) {
        this.nowMillisSupplier = Objects.requireNonNull(nowMillisSupplier, "nowMillisSupplier");
    }

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        BreakthroughCinematicPayload payload = BreakthroughCinematicPayload.parse(envelope.payload());
        if (payload == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring breakthrough_cinematic payload: required fields missing or invalid"
            );
        }

        long nowMillis = nowMillisSupplier.getAsLong();
        BreakthroughSpectacleRenderer.SpectaclePlan plan =
            BreakthroughSpectacleRenderer.plan(payload, SeasonStateStore.snapshot(), nowMillis);
        VisualEffectState visualEffectState = VisualEffectState.create(
            plan.visualEffectType(),
            plan.visualIntensity(),
            plan.visualDurationMillis(),
            nowMillis
        );
        ServerDataDispatch.ToastSpec toastSpec =
            new ServerDataDispatch.ToastSpec(plan.toastText(), plan.toastColor(), 2_400L);

        return ServerDataDispatch.handledWithEventAlert(
            envelope.type(),
            toastSpec,
            visualEffectState,
            "breakthrough_cinematic accepted phase=" + payload.phase().wireName()
                + " realm=" + payload.realmFrom() + "->" + payload.realmTo()
                + " vfx=" + String.join(",", plan.vfxEventIds())
                + " audio=" + plan.audioRecipeId()
                + " anim=" + plan.animationId()
        );
    }
}
