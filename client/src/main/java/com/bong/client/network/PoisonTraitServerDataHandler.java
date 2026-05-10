package com.bong.client.network;

import com.bong.client.hud.PoisonTraitHudStateStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.Objects;
import java.util.function.LongSupplier;

public final class PoisonTraitServerDataHandler implements ServerDataHandler {
    private static final long LIFESPAN_WARNING_DURATION_MILLIS = 1_500L;

    private final LongSupplier nowMillis;

    public PoisonTraitServerDataHandler() {
        this(System::currentTimeMillis);
    }

    PoisonTraitServerDataHandler(LongSupplier nowMillis) {
        this.nowMillis = Objects.requireNonNull(nowMillis, "nowMillis");
    }

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return switch (envelope.type()) {
            case "poison_trait_state" -> handleState(envelope);
            case "poison_dose_event" -> handleDose(envelope);
            case "poison_overdose_event" -> handleOverdose(envelope);
            default -> ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring poison trait payload: unsupported type '" + envelope.type() + "'"
            );
        };
    }

    private ServerDataDispatch handleState(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        Long playerEntityId = readNonNegativeLong(payload, "player_entity_id");
        Float toxicity = readRangedFloat(payload, "poison_toxicity", 0.0f, 100.0f);
        Float digestionCurrent = readNonNegativeFloat(payload, "digestion_current");
        Float digestionCapacity = readPositiveFloat(payload, "digestion_capacity");
        Boolean toxicityTierUnlocked = readBool(payload, "toxicity_tier_unlocked");
        if (playerEntityId == null
            || toxicity == null
            || digestionCurrent == null
            || digestionCapacity == null
            || toxicityTierUnlocked == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring poison_trait_state payload: required fields missing or invalid"
            );
        }

        PoisonTraitHudStateStore.State previous = PoisonTraitHudStateStore.snapshot();
        boolean active = toxicity > 0.0f || digestionCurrent > 0.0f || toxicityTierUnlocked;
        PoisonTraitHudStateStore.update(new PoisonTraitHudStateStore.State(
            active,
            toxicity,
            digestionCurrent,
            digestionCapacity,
            previous.lifespanWarningUntilMillis(),
            previous.lifespanYearsLost()
        ));
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied poison_trait_state (toxicity=" + toxicity + " digestion=" + digestionCurrent + ")"
        );
    }

    private ServerDataDispatch handleDose(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        Long playerEntityId = readNonNegativeLong(payload, "player_entity_id");
        Float toxicity = readRangedFloat(payload, "poison_level_after", 0.0f, 100.0f);
        Float digestionCurrent = readNonNegativeFloat(payload, "digestion_after");
        if (playerEntityId == null || toxicity == null || digestionCurrent == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring poison_dose_event payload: required fields missing or invalid"
            );
        }

        PoisonTraitHudStateStore.State previous = PoisonTraitHudStateStore.snapshot();
        PoisonTraitHudStateStore.update(new PoisonTraitHudStateStore.State(
            true,
            toxicity,
            digestionCurrent,
            previous.digestionCapacity(),
            previous.lifespanWarningUntilMillis(),
            previous.lifespanYearsLost()
        ));
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied poison_dose_event (toxicity=" + toxicity + " digestion=" + digestionCurrent + ")"
        );
    }

    private ServerDataDispatch handleOverdose(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        Long playerEntityId = readNonNegativeLong(payload, "player_entity_id");
        Float lifespanPenaltyYears = readNonNegativeFloat(payload, "lifespan_penalty_years");
        if (playerEntityId == null || lifespanPenaltyYears == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring poison_overdose_event payload: required fields missing or invalid"
            );
        }

        PoisonTraitHudStateStore.State previous = PoisonTraitHudStateStore.snapshot();
        PoisonTraitHudStateStore.update(new PoisonTraitHudStateStore.State(
            true,
            previous.toxicity(),
            previous.digestionCurrent(),
            previous.digestionCapacity(),
            nowMillis.getAsLong() + LIFESPAN_WARNING_DURATION_MILLIS,
            lifespanPenaltyYears
        ));
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied poison_overdose_event (lifespan_penalty_years=" + lifespanPenaltyYears + ")"
        );
    }

    private static Long readNonNegativeLong(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) return null;
        double value = primitive.getAsDouble();
        if (!Double.isFinite(value) || value < 0.0 || value > Long.MAX_VALUE || Math.floor(value) != value) return null;
        return (long) value;
    }

    private static Float readPositiveFloat(JsonObject object, String fieldName) {
        Float value = readNonNegativeFloat(object, fieldName);
        if (value == null || value <= 0.0f) return null;
        return value;
    }

    private static Float readNonNegativeFloat(JsonObject object, String fieldName) {
        return readRangedFloat(object, fieldName, 0.0f, Float.MAX_VALUE);
    }

    private static Float readRangedFloat(JsonObject object, String fieldName, float min, float max) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) return null;
        double value = primitive.getAsDouble();
        if (!Double.isFinite(value) || value < min || value > max) return null;
        return (float) value;
    }

    private static Boolean readBool(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isBoolean() ? primitive.getAsBoolean() : null;
    }
}
