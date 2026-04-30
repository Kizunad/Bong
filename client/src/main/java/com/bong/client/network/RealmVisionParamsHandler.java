package com.bong.client.network;

import com.bong.client.hud.BongToast;
import com.bong.client.visual.realm_vision.ClientRenderDistanceAdvisor;
import com.bong.client.visual.realm_vision.ClientRenderDistanceAdvisorState;
import com.bong.client.visual.realm_vision.RealmVisionState;
import com.bong.client.visual.realm_vision.RealmVisionStateReducer;
import com.bong.client.visual.realm_vision.RealmVisionStateStore;
import com.google.gson.JsonObject;
import net.minecraft.client.MinecraftClient;

public final class RealmVisionParamsHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        RealmVisionState next = RealmVisionStateReducer.apply(
            RealmVisionStateStore.snapshot(),
            payload,
            System.currentTimeMillis() / 50L
        );
        RealmVisionStateStore.replace(next);
        adviseRenderDistance();
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied realm_vision_params (fog_start=" + next.current().fogStart()
                + ", chunks=" + next.serverViewDistanceChunks() + ")"
        );
    }

    private static void adviseRenderDistance() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.options == null) {
            return;
        }
        int chunks = client.options.getViewDistance().getValue();
        if (ClientRenderDistanceAdvisorState.markWarnedIfNeeded(chunks)) {
            BongToast.show(ClientRenderDistanceAdvisor.warningText(), 0xFFE8D080, System.currentTimeMillis(), 4000L);
        }
    }
}
