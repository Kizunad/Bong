package com.bong.client.network;

import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import com.google.gson.JsonElement;

/**
 * 神识感知矿脉回执处理器（plan-exploration-probe-return-v1 P0）。
 * <p>
 * 处理 type="mineral_probe_result" 的 S2C payload：
 * <ul>
 *   <li>Found → 结构化 actionbar/SFX feedback spec（按丰度上色）</li>
 *   <li>Denied → 结构化 actionbar/SFX feedback spec（按 denial_reason 显示对应文案）</li>
 * </ul>
 *
 * 视听规格见 plan P0 §视听规格：
 * <ul>
 *   <li>Found 颜色：remaining_units &gt; 50 → #6EE7B7（青）；10-50 → #FCD34D（琥珀）；&lt;10 → #F87171（赤）</li>
 *   <li>Found SFX：{@code block.amethyst_block.chime} pitch=1.4 / volume=0.3</li>
 *   <li>Denied SFX：{@code block.note_block.bass} pitch=0.6 / volume=0.2</li>
 * </ul>
 */
public final class MineralProbeResultHandler implements ServerDataHandler {

    // ─── 丰度阈值 ─────────────────────────────────────────────────
    private static final int ABUNDANT_THRESHOLD = 50;
    private static final int SPARSE_THRESHOLD   = 10;

    // ─── 颜色十六进制（与 plan P0 规格一致）────────────────────────
    /** >50 缕：青绿（丰盛）*/
    private static final int COLOR_ABUNDANT = 0x6EE7B7;
    /** 10-50 缕：琥珀（适中）*/
    private static final int COLOR_MODERATE = 0xFCD34D;
    /** <10 缕：赤（稀薄）*/
    private static final int COLOR_SPARSE   = 0xF87171;
    /** Denied 灰字 */
    private static final int COLOR_DENIED   = 0x9CA3AF;

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String kind = readString(payload, "kind");
        if (kind == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "mineral_probe_result: 'kind' 字段缺失，payload 忽略"
            );
        }

        if ("found".equals(kind)) {
            return handleFound(envelope, payload);
        } else if ("denied".equals(kind)) {
            return handleDenied(envelope, payload);
        } else {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "mineral_probe_result: 未知 kind='" + kind + "'，忽略"
            );
        }
    }

    // ─── Found ────────────────────────────────────────────────────

    private ServerDataDispatch handleFound(ServerDataEnvelope envelope, JsonObject payload) {
        String displayNameZh = readString(payload, "display_name_zh");
        int remainingUnits   = readInt(payload, "remaining_units", 0);

        String name = (displayNameZh != null && !displayNameZh.isBlank())
            ? displayNameZh
            : "灵脉";

        String overlayText = foundOverlayText(name, remainingUnits);
        int color = colorByAbundance(remainingUnits);
        MineralProbeFeedbackSpec feedback = new MineralProbeFeedbackSpec(
            overlayText,
            color,
            MineralProbeFeedbackSpec.SoundEffect.AMETHYST_CHIME,
            0.3f,
            1.4f
        );

        return ServerDataDispatch.handledWithMineralProbeFeedback(
            envelope.type(),
            feedback,
            "神识感知矿脉 found: " + name + " remaining=" + remainingUnits
        );
    }

    // ─── Denied ───────────────────────────────────────────────────

    private ServerDataDispatch handleDenied(ServerDataEnvelope envelope, JsonObject payload) {
        String reason = readString(payload, "denial_reason");
        String message = denialMessage(reason);

        MineralProbeFeedbackSpec feedback = new MineralProbeFeedbackSpec(
            message,
            COLOR_DENIED,
            MineralProbeFeedbackSpec.SoundEffect.NOTE_BLOCK_BASS,
            0.2f,
            0.6f
        );

        return ServerDataDispatch.handledWithMineralProbeFeedback(
            envelope.type(),
            feedback,
            "神识感知矿脉 denied: " + reason
        );
    }

    // ─── helpers ─────────────────────────────────────────────────

    /** 按剩余储量返回丰度颜色（plan P0 视听规格）。package-private 供测试直接断言。 */
    static int colorByAbundance(int remaining) {
        if (remaining > ABUNDANT_THRESHOLD) {
            return COLOR_ABUNDANT;
        } else if (remaining >= SPARSE_THRESHOLD) {
            return COLOR_MODERATE;
        } else {
            return COLOR_SPARSE;
        }
    }

    private static String foundOverlayText(String name, int remainingUnits) {
        return "「" + name + "」灵脉 · 余 " + remainingUnits + " 缕";
    }

    /**
     * 按 denial_reason snake_case → 显示文案（plan P0 Denied 显示规格）。
     * 未知 reason 退回兜底文案。
     */
    static String denialMessage(String reason) {
        if (reason == null) {
            return "灵脉模糊，难以辨形";
        }
        return switch (reason) {
            case "realm_too_low"         -> "神识未及，凝脉方可感矿脉";
            case "out_of_range"          -> "神识探之不及";
            case "not_mineral_ore"       -> "此处并无灵脉";
            case "stale_ore_index",
                 "mineral_not_registered" -> "灵脉模糊，难以辨形";
            default                      -> "灵脉模糊，难以辨形";
        };
    }

    private static String readString(JsonObject object, String fieldName) {
        JsonPrimitive prim = readPrimitive(object, fieldName);
        if (prim == null || !prim.isString()) return null;
        return prim.getAsString();
    }

    private static int readInt(JsonObject object, String fieldName, int fallback) {
        JsonPrimitive prim = readPrimitive(object, fieldName);
        if (prim == null || !prim.isNumber()) return fallback;
        return prim.getAsInt();
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement el = object.get(fieldName);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        return el.getAsJsonPrimitive();
    }
}
