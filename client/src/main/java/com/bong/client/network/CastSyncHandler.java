package com.bong.client.network;

import com.bong.client.combat.CastOutcome;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * plan-HUD-v1 §4 cast 状态机 client handler。
 * 收到 server 推的 cast_sync 后整体替换 {@link CastStateStore}，
 * cast bar planner 据此渲染 / 隐藏。
 */
public final class CastSyncHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String phaseStr = readString(payload, "phase");
        Long slot = readLong(payload, "slot");
        Long durationMs = readLong(payload, "duration_ms");
        Long startedAtMs = readLong(payload, "started_at_ms");
        String outcomeStr = readString(payload, "outcome");
        if (phaseStr == null || slot == null || durationMs == null
            || startedAtMs == null || outcomeStr == null
            || slot < 0 || slot > 8 || durationMs < 0) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring cast_sync payload: required fields missing or invalid"
            );
        }

        CastState.Source source = sourceFor(slot.intValue());
        CastState next = switch (phaseStr) {
            case "idle" -> CastState.idle();
            case "casting" -> CastState.casting(source, slot.intValue(), durationMs.intValue(), startedAtMs);
            case "complete" -> CastState
                .casting(source, slot.intValue(), durationMs.intValue(), startedAtMs)
                .transitionToComplete(System.currentTimeMillis());
            case "interrupt" -> CastState
                .casting(source, slot.intValue(), durationMs.intValue(), startedAtMs)
                .transitionToInterrupt(parseOutcome(outcomeStr), System.currentTimeMillis());
            default -> null;
        };
        if (next == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring cast_sync payload: unknown phase '" + phaseStr + "'"
            );
        }
        CastStateStore.replace(next);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied cast_sync (phase=" + phaseStr + " slot=" + slot
                + " outcome=" + outcomeStr + ")"
        );
    }

    private static CastState.Source sourceFor(int slot) {
        CastState current = CastStateStore.snapshot();
        if (current.slot() == slot && current.source() == CastState.Source.SKILL_BAR) {
            return CastState.Source.SKILL_BAR;
        }
        return CastState.Source.QUICK_SLOT;
    }

    private static CastOutcome parseOutcome(String wire) {
        return switch (wire) {
            case "completed" -> CastOutcome.COMPLETED;
            case "interrupt_movement" -> CastOutcome.INTERRUPT_MOVEMENT;
            case "interrupt_contam" -> CastOutcome.INTERRUPT_CONTAM;
            case "interrupt_control" -> CastOutcome.INTERRUPT_CONTROL;
            case "user_cancel" -> CastOutcome.USER_CANCEL;
            case "death" -> CastOutcome.DEATH;
            default -> CastOutcome.NONE;
        };
    }

    private static String readString(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isString() ? primitive.getAsString() : null;
    }

    private static Long readLong(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isNumber() ? primitive.getAsLong() : null;
    }
}
