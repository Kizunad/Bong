package com.bong.client.network;

import com.bong.client.state.VisualEffectState;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.Objects;
import java.util.function.LongSupplier;

public final class KnockbackSyncHandler implements ServerDataHandler {
    static final long TICK_MILLIS = 50L;
    static final long MIN_DURATION_MILLIS = 90L;

    private final LongSupplier nowMillisSupplier;

    public KnockbackSyncHandler() {
        this(System::currentTimeMillis);
    }

    KnockbackSyncHandler(LongSupplier nowMillisSupplier) {
        this.nowMillisSupplier = Objects.requireNonNull(nowMillisSupplier, "nowMillisSupplier");
    }

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        double distanceBlocks = readDouble(payload, "distance_blocks", 0.0);
        double collisionDamage = readDouble(payload, "collision_damage", 0.0);
        boolean blockBroken = readBoolean(payload, "block_broken", false);
        long durationTicks = Math.max(1L, readLong(payload, "duration_ticks", 1L));

        double intensity = clamp(
            distanceBlocks / 8.0 + collisionDamage / 40.0 + (blockBroken ? 0.2 : 0.0),
            0.1,
            1.0
        );
        long durationMillis = Math.max(MIN_DURATION_MILLIS, durationTicks * TICK_MILLIS);
        VisualEffectState effect = VisualEffectState.create(
            "hit_pushback",
            intensity,
            durationMillis,
            nowMillisSupplier.getAsLong()
        );

        return ServerDataDispatch.handledWithEventAlert(
            envelope.type(),
            null,
            effect,
            "Applied knockback_sync distance=" + distanceBlocks + " duration_ticks=" + durationTicks
        );
    }

    private static double readDouble(JsonObject payload, String field, double fallback) {
        JsonElement element = payload.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return fallback;
        }
        try {
            double value = element.getAsDouble();
            return Double.isFinite(value) ? value : fallback;
        } catch (RuntimeException ignored) {
            return fallback;
        }
    }

    private static long readLong(JsonObject payload, String field, long fallback) {
        JsonElement element = payload.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return fallback;
        }
        try {
            return element.getAsLong();
        } catch (RuntimeException ignored) {
            return fallback;
        }
    }

    private static boolean readBoolean(JsonObject payload, String field, boolean fallback) {
        JsonElement element = payload.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return fallback;
        }
        try {
            return element.getAsBoolean();
        } catch (RuntimeException ignored) {
            return fallback;
        }
    }

    private static double clamp(double value, double min, double max) {
        if (!Double.isFinite(value)) {
            return min;
        }
        return Math.max(min, Math.min(max, value));
    }
}
