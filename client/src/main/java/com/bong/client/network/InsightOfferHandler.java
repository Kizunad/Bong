package com.bong.client.network;

import com.bong.client.insight.InsightAlignment;
import com.bong.client.insight.InsightCategory;
import com.bong.client.insight.InsightChoice;
import com.bong.client.insight.InsightOfferStore;
import com.bong.client.insight.InsightOfferViewModel;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * 修炼顿悟邀约处理器（plan-exploration-probe-return-v1 P2）。
 *
 * <p>处理 type="insight_offer" 的 S2C payload，将 InsightOfferV1（服务端内部形状）
 * 合成为 InsightOfferViewModel / InsightChoice 填入 InsightOfferStore。
 *
 * <p>⚠️ 不是 HeartDemonOfferHandler 的盲拷：InsightChoiceV1 形状不同：
 * <ul>
 *   <li>无 choice_id / title / effect_summary / style_hint</li>
 *   <li>有 effect_kind / magnitude / flavor_text / cost_* 可选字段</li>
 * </ul>
 *
 * <p>合成规则（见 plan P2 §交付物）：
 * <ul>
 *   <li>{@code choiceId = "insight_choice_" + i}</li>
 *   <li>{@code title} 由 category + effect_kind 派生</li>
 *   <li>{@code effectSummary} 由 effect_kind + magnitude（+ cost_*）组词</li>
 *   <li>{@code flavor = flavor_text}</li>
 *   <li>{@code alignment = InsightAlignment.parse(alignment)}</li>
 *   <li>InsightOfferV1 不含 trigger_label / realm_label / composure / quota_*，
 *       统一用 fallback 默认值（顿悟语境）</li>
 * </ul>
 */
public final class InsightOfferHandler implements ServerDataHandler {

    private static final Logger LOGGER = LoggerFactory.getLogger(InsightOfferHandler.class);

    /** 顿悟开屏 fallback：trigger_label 默认值（顿悟语境，区别心魔"危"）。 */
    private static final String DEFAULT_TRIGGER_LABEL = "顿悟临身";
    /** 顿悟开屏 fallback：realm_label。 */
    private static final String DEFAULT_REALM_LABEL   = "道机一现";
    /** 顿悟 composure 默认：当前无心魔压力，取中性值。 */
    private static final double DEFAULT_COMPOSURE     = 0.75;
    /** 顿悟额度默认：境界内至少还有一次机会。 */
    private static final int    DEFAULT_QUOTA_REMAINING = 1;
    private static final int    DEFAULT_QUOTA_TOTAL     = 3;
    /** 顿悟默认超时：90 秒（玩家需在此内做出选择）。 */
    private static final long   DEFAULT_TIMEOUT_MILLIS  = 90_000L;

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();

        String triggerId = readString(payload, "trigger_id");
        if (triggerId == null || triggerId.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "insight_offer: 'trigger_id' 缺失，payload 忽略"
            );
        }

        List<InsightChoice> choices = readChoices(payload);
        if (choices.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "insight_offer: choices 为空，payload 忽略（trigger_id=" + triggerId + "）"
            );
        }

        long nowMillis = System.currentTimeMillis();
        long expiresAtMillis = nowMillis + DEFAULT_TIMEOUT_MILLIS;

        InsightOfferStore.replace(new InsightOfferViewModel(
            triggerId,
            DEFAULT_TRIGGER_LABEL,
            DEFAULT_REALM_LABEL,
            DEFAULT_COMPOSURE,
            DEFAULT_QUOTA_REMAINING,
            DEFAULT_QUOTA_TOTAL,
            expiresAtMillis,
            choices
        ));

        return ServerDataDispatch.handled(
            envelope.type(),
            "顿悟 insight_offer 已推入 InsightOfferStore trigger_id='" + triggerId
                + "' choices=" + choices.size()
        );
    }

    // ─── 解析 choices[] ───────────────────────────────────────────

    private static List<InsightChoice> readChoices(JsonObject payload) {
        JsonElement choicesElement = payload.get("choices");
        if (choicesElement == null || !choicesElement.isJsonArray()) {
            return List.of();
        }
        JsonArray array = choicesElement.getAsJsonArray();
        if (array.size() > 3) {
            LOGGER.warn("insight_offer: choices={} 超过 maxItems=3，截断到 3（server schema 应已限制）", array.size());
        }
        List<InsightChoice> result = new ArrayList<>();
        int limit = Math.min(3, array.size());
        for (int i = 0; i < limit; i++) {
            JsonElement el = array.get(i);
            if (!el.isJsonObject()) {
                continue;
            }
            JsonObject c = el.getAsJsonObject();

            String category   = readString(c, "category");
            String effectKind = fallback(readString(c, "effect_kind"), "unknown");
            double magnitude  = readDouble(c, "magnitude", 0.0);
            String flavorText = fallback(readString(c, "flavor_text"), "道机浮现。");
            String alignment  = readString(c, "alignment");
            String costKind   = readString(c, "cost_kind");
            double costMagnitude = readDouble(c, "cost_magnitude", 0.0);
            String costFlavor = readString(c, "cost_flavor");

            String choiceId     = "insight_choice_" + i;
            String title        = deriveTitle(category, effectKind);
            String effectSummary = deriveEffectSummary(effectKind, magnitude);
            String costSummary  = deriveCostSummary(costKind, costMagnitude);
            String finalCostFlavor = fallback(costFlavor, "气机微有变动。");
            String styleHint    = deriveStyleHint(alignment);

            result.add(new InsightChoice(
                choiceId,
                parseCategory(category),
                InsightAlignment.parse(alignment),
                title,
                effectSummary,
                costSummary,
                flavorText,
                finalCostFlavor,
                styleHint
            ));
        }
        return result;
    }

    // ─── 合成 title（category + effect_kind → 短标题）────────────

    /**
     * 从 category + effect_kind 派生一行简短标题。
     * 格式：「{category中文} · {effect_kind 末词}」（如 "心境 · 回复"）。
     */
    static String deriveTitle(String category, String effectKind) {
        String catZh = categoryZh(category);
        String effectTail = effectKindTail(effectKind);
        return catZh + " · " + effectTail;
    }

    private static String categoryZh(String category) {
        if (category == null) return "感悟";
        return switch (category) {
            case "Meridian"     -> "经脉";
            case "Qi"           -> "真元";
            case "Composure"    -> "心境";
            case "Coloring"     -> "染色";
            case "Breakthrough" -> "突破";
            case "Style"        -> "流派";
            case "Perception"   -> "感知";
            default             -> "感悟";
        };
    }

    private static String effectKindTail(String effectKind) {
        if (effectKind == null || effectKind.isBlank()) return "感悟";
        // 取 snake_case 末节（最后一段，去掉前缀）
        String[] parts = effectKind.split("_");
        return switch (parts[parts.length - 1]) {
            case "rate"          -> "通速";
            case "discount"      -> "折损";
            case "tolerance"     -> "耐度";
            case "factor"        -> "倍率";
            case "efficiency"    -> "效率";
            case "max"           -> "解封";
            case "recover"       -> "回复";
            case "bonus"         -> "增幅";
            case "drop"          -> "降门";
            case "window"        -> "预判";
            case "affinity"      -> "亲和";
            case "concealment"   -> "隐匿";
            case "disenchant"    -> "拆解";
            case "resist"        -> "抗性";
            case "speed"         -> "流速";
            case "practice"      -> "心法";
            case "perception"    -> "感知";
            case "enlightenment" -> "顿悟";
            case "add"           -> "增量";
            case "breakthrough"  -> "感悟";
            default              -> "感悟";
        };
    }

    // ─── 合成 effectSummary ───────────────────────────────────────

    /** 从 effect_kind + magnitude 组词一行数值描述。 */
    static String deriveEffectSummary(String effectKind, double magnitude) {
        if (effectKind == null || effectKind.isBlank()) {
            return "效果待结算";
        }
        if (magnitude == 0.0) {
            return effectKind + "（无幅度）";
        }
        // 百分比格式（保留一位小数，Locale.ROOT 防地区漂移）
        String pct = String.format(Locale.ROOT, "%.1f%%", magnitude * 100);
        return effectKind + " +" + pct;
    }

    // ─── 合成 costSummary ─────────────────────────────────────────

    /** 从 cost_kind + cost_magnitude 组词代价短描述。 */
    static String deriveCostSummary(String costKind, double costMagnitude) {
        if (costKind == null || costKind.isBlank()) {
            return "代价待结算";
        }
        if (costMagnitude == 0.0) {
            return costKind;
        }
        String pct = String.format(Locale.ROOT, "%.1f%%", costMagnitude * 100);
        return costKind + " +" + pct;
    }

    // ─── 合成 styleHint ───────────────────────────────────────────

    private static String deriveStyleHint(String alignment) {
        if (alignment == null) return "通用";
        return switch (alignment) {
            case "converge" -> "深化";
            case "diverge"  -> "转向";
            default         -> "通用";
        };
    }

    // ─── 解析 InsightCategory ────────────────────────────────────

    private static InsightCategory parseCategory(String wire) {
        if (wire == null) return InsightCategory.COMPOSURE;
        return switch (wire) {
            case "Meridian"     -> InsightCategory.MERIDIAN;
            case "Qi"           -> InsightCategory.QI;
            case "Composure"    -> InsightCategory.COMPOSURE;
            case "Coloring"     -> InsightCategory.QI_COLOR;
            case "Breakthrough" -> InsightCategory.BREAKTHROUGH;
            case "Style"        -> InsightCategory.SCHOOL;
            case "Perception"   -> InsightCategory.PERCEPTION;
            default             -> InsightCategory.COMPOSURE;
        };
    }

    // ─── JSON 读取 helpers ────────────────────────────────────────

    private static String readString(JsonObject obj, String field) {
        JsonPrimitive prim = readPrimitive(obj, field);
        if (prim == null || !prim.isString()) return null;
        return prim.getAsString();
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        JsonPrimitive prim = readPrimitive(obj, field);
        if (prim == null || !prim.isNumber()) return fallback;
        double v = prim.getAsDouble();
        return Double.isFinite(v) ? v : fallback;
    }

    private static JsonPrimitive readPrimitive(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        return el.getAsJsonPrimitive();
    }

    private static String fallback(String value, String fallback) {
        return value == null || value.isBlank() ? fallback : value;
    }
}
