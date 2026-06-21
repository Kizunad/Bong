package com.bong.client.network;

import com.bong.client.combat.CastOutcome;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
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
        CastOutcome outcome = parseOutcome(outcomeStr);
        CastState next = switch (phaseStr) {
            // idle + non-NONE outcome = 施放前被拒绝（如 MeridianGated）：
            // 服务端用 Idle 表达"没有进行中 cast 被打断，是施放前拒绝"。
            // 客户端需要显示拒绝反馈，通过合成一个 0ms casting 再 transitionToInterrupt 实现。
            case "idle" -> outcome == CastOutcome.NONE
                ? CastState.idle()
                : CastState.casting(source, slot.intValue(), 0, startedAtMs)
                    .transitionToInterrupt(outcome, System.currentTimeMillis());
            case "casting" -> CastState.casting(source, slot.intValue(), durationMs.intValue(), startedAtMs);
            case "complete" -> CastState
                .casting(source, slot.intValue(), durationMs.intValue(), startedAtMs)
                .transitionToComplete(System.currentTimeMillis());
            case "interrupt" -> CastState
                .casting(source, slot.intValue(), durationMs.intValue(), startedAtMs)
                .transitionToInterrupt(outcome, System.currentTimeMillis());
            default -> null;
        };
        if (next == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring cast_sync payload: unknown phase '" + phaseStr + "'"
            );
        }
        CastStateStore.replace(next);
        // 通用技能警示 HUD：施放前被拒（经脉门控 + 所有 resolver Reject*）时，
        // 把映射好的中文文案推进右侧统一事件流——瞬态（P2，~4s 自动过期），
        // 复用既有事件流渲染，不新增常驻 HUD。对所有技能的所有拒绝原因生效。
        publishWarningIfRejected(outcome);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied cast_sync (phase=" + phaseStr + " slot=" + slot
                + " outcome=" + outcomeStr + ")"
        );
    }

    /**
     * 把施法被拒原因弹成瞬态警示。非拒绝 outcome（如 none/completed/interrupt_*）跳过，
     * 保证只有"前置不满足导致没放出来"才提示。文案由 {@link CastOutcome#warningText()}
     * 统一映射（枚举驱动，覆盖所有拒绝原因，不 hardcode 单招）。
     */
    private static void publishWarningIfRejected(CastOutcome outcome) {
        if (outcome == null || !outcome.isCastRejection()) {
            return;
        }
        String text = outcome.warningText();
        if (text == null || text.isEmpty()) {
            return;
        }
        UnifiedEventStore.stream().publish(
            UnifiedEvent.Channel.SYSTEM,
            UnifiedEvent.Priority.P2_NORMAL,
            // 同 source_tag 让 1.5s 内重复按键折叠成 "×N"，避免连点刷屏。
            "skill_warn",
            text,
            // 琥珀/橙色——拒绝警示需区别于 SYSTEM 频道默认绿色(绿=正向反馈，与"被拒"语义相悖)。
            0xFFFFAA40,
            System.currentTimeMillis()
        );
    }

    private static CastState.Source sourceFor(int slot) {
        CastState current = CastStateStore.snapshot();
        if (current.isCasting() && current.slot() == slot && current.source() == CastState.Source.SKILL_BAR) {
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
            // 服务端 CastOutcomeV1::MeridianGated serde(rename_all="snake_case") → "meridian_gated"。
            // proto 路径经 ProtoServerDataBridge.bridgeCastSync 把 CAST_OUTCOME_* 前缀剥成同样的
            // snake_case，两条 wire 形态统一。
            case "meridian_gated" -> CastOutcome.MERIDIAN_GATED;
            // 通用技能警示（plan-skill-warn-hud）：resolver 拒绝原因。
            case "reject_qi_insufficient" -> CastOutcome.REJECT_QI_INSUFFICIENT;
            case "reject_on_cooldown" -> CastOutcome.REJECT_ON_COOLDOWN;
            case "reject_invalid_target" -> CastOutcome.REJECT_INVALID_TARGET;
            case "reject_in_recovery" -> CastOutcome.REJECT_IN_RECOVERY;
            case "reject_realm_too_low" -> CastOutcome.REJECT_REALM_TOO_LOW;
            case "reject_no_weapon" -> CastOutcome.REJECT_NO_WEAPON;
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
