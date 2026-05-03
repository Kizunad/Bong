package com.bong.client.combat.handler;

import com.bong.client.combat.store.DerivedAttrsStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * Handles {@code false_skin_state} payloads from plan-tuike-v1.
 */
public final class FalseSkinStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        int layers = (int) Math.max(0L, Math.round(readDouble(payload, "layers_remaining", 0d)));
        DerivedAttrsStore.State current = DerivedAttrsStore.snapshot();
        DerivedAttrsStore.replace(new DerivedAttrsStore.State(
            current.flying(),
            current.flyingQiRemaining(),
            current.flyingForceDescentAtMs(),
            current.phasing(),
            current.phasingUntilMs(),
            current.tribulationLocked(),
            current.tribulationStage(),
            current.throughputPeakNorm(),
            layers,
            current.vortexActive()
        ));
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied false_skin_state (layers_remaining=" + layers + ")"
        );
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double v = p.getAsDouble();
        return Double.isFinite(v) ? v : fallback;
    }
}
