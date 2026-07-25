package com.bong.client.fauna;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.util.Collections;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * plan-devour-rat-model P2 —— 噬元鼠吸元档位接收端。
 *
 * <p>噬元鼠靠咬人吸真元（服务端 {@code RatBlackboard.drained_qi}）。服务端把储量离散成
 * 0/1/2 三档下发，渲染层据此选贴图变体 {@code devour_rat_q{0,1,2}.png}
 * （尾脊蓝填充 0 / 半 / 全）并由 {@link FaunaEmissiveGlowLayer} 让蓝脊 + 红眼发光。
 *
 * <p>channel {@code bong:rat_qi_tier}，wire payload：
 * <pre>{@code
 * {
 *   "v": 1,
 *   "type": "rat_qi_tier",
 *   "entries": [{"id": 42, "tier": 2}, {"id": 77, "tier": 1}]
 * }
 * }</pre>
 *
 * <p><b>缺省即 0 档</b>：服务端只下发 {@code tier > 0} 的鼠（绝大多数鼠一口没咬过），
 * 查不到就按 0 档渲染。每次 sync 是**全量替换**——某只鼠从名单里消失（死亡 / 离开视野 /
 * 储量归还 zone）即自动降回 0 档，无需额外的"降档"事件。
 *
 * <p>线程安全：{@link #handleSync} 由 {@code BongNetworkHandler} 经
 * {@code client.execute(...)} 派回主（渲染）线程执行，渲染层读也在同一线程，故正常路径下
 * 并无跨线程竞争。{@link #TIERS} 仍用 {@link ConcurrentHashMap} 兜底（断线清理等旁路可能
 * 从别处调用），全量替换用逐 key diff 而非 clear+putAll——即便只是同线程，clear 之后到
 * putAll 之前若插入任何读取（将来有人加个 hook / 换成异步派发）都会把整群鼠闪回 q0。
 */
public final class RatQiTierHandler {

    /** {@code bong:rat_qi_tier} channel identifier components。 */
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "rat_qi_tier";

    /** wire {@code type} 字段值；与服务端 {@code RatQiTierS2c::sync} 逐字对齐。 */
    public static final String PAYLOAD_TYPE = "rat_qi_tier";

    /** 档位上限；与服务端 {@code RAT_QI_TIER_MAX} 同值（贴图只有 q0/q1/q2）。 */
    public static final int TIER_MAX = 2;

    /** 未收到条目时的缺省档位。 */
    public static final int TIER_DEFAULT = 0;

    /** MC 协议 entity id → 吸元档位（只存 {@code tier > 0}）。 */
    private static final Map<Integer, Integer> TIERS = new ConcurrentHashMap<>();

    private RatQiTierHandler() {
    }

    /**
     * 处理 {@code bong:rat_qi_tier} payload：全量替换当前档位表。
     *
     * @return {@code true} 表示 payload 有效且已处理；{@code false} 表示格式错误（现态不变）
     */
    public static boolean handleSync(String jsonPayload, int payloadSizeBytes) {
        if (jsonPayload == null
            || payloadSizeBytes < 0
            || payloadSizeBytes > ServerDataEnvelope.MAX_PAYLOAD_BYTES) {
            return false;
        }
        JsonObject root;
        try {
            root = JsonParser.parseString(jsonPayload).getAsJsonObject();
        } catch (RuntimeException e) {
            return false;
        }
        if (intField(root, "v", -1) != 1) {
            return false;
        }
        if (!PAYLOAD_TYPE.equals(stringField(root, "type"))) {
            return false;
        }

        Map<Integer, Integer> parsed = parseEntries(root);
        // 全量替换：先写入/更新新值，再删掉本轮缺席的 key（缺席 = 降回 0 档）。
        // 不用 clear()+putAll()——渲染线程可能正好落在空窗里，整群鼠会闪一帧 q0。
        TIERS.putAll(parsed);
        TIERS.keySet().removeIf(id -> !parsed.containsKey(id));
        return true;
    }

    /**
     * 查询某只鼠当前的吸元档位。
     *
     * @param entityId MC 协议 entity id
     * @return 0..={@link #TIER_MAX}；未收到条目返回 {@link #TIER_DEFAULT}
     */
    public static int tierOf(int entityId) {
        return TIERS.getOrDefault(entityId, TIER_DEFAULT);
    }

    /** 当前档位表的只读快照（测试用）。 */
    public static Map<Integer, Integer> snapshot() {
        return Collections.unmodifiableMap(new HashMap<>(TIERS));
    }

    /** 断线时清空，防止跨 session 状态泄漏（entity id 会被重新分配给别的实体）。 */
    public static void clearOnDisconnect() {
        TIERS.clear();
    }

    // ── 内部 ─────────────────────────────────────────────────────────────────

    private static Map<Integer, Integer> parseEntries(JsonObject root) {
        if (!root.has("entries") || root.get("entries").isJsonNull()) {
            return Map.of();
        }
        JsonElement element = root.get("entries");
        if (!element.isJsonArray()) {
            return Map.of();
        }
        JsonArray array = element.getAsJsonArray();
        Map<Integer, Integer> result = new HashMap<>(array.size());
        for (JsonElement item : array) {
            if (item == null || !item.isJsonObject()) {
                continue;
            }
            JsonObject entry = item.getAsJsonObject();
            int id = intField(entry, "id", Integer.MIN_VALUE);
            if (id == Integer.MIN_VALUE) {
                continue;
            }
            int tier = clampTier(intField(entry, "tier", TIER_DEFAULT));
            if (tier <= TIER_DEFAULT) {
                // 0 档不占表（与服务端"缺省即 0 档"约定一致）；
                // 服务端本不该发，收到也不能让它把表撑大。
                continue;
            }
            result.put(id, tier);
        }
        return result;
    }

    /** 越界档位夹到 [0, {@link #TIER_MAX}]——贴图只有三张，脏值不能变成 missing texture。 */
    public static int clampTier(int tier) {
        return Math.max(TIER_DEFAULT, Math.min(TIER_MAX, tier));
    }

    private static int intField(JsonObject root, String fieldName, int fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            return root.get(fieldName).getAsInt();
        } catch (RuntimeException e) {
            return fallback;
        }
    }

    private static String stringField(JsonObject root, String fieldName) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return "";
        }
        try {
            String value = root.get(fieldName).getAsString();
            return value == null ? "" : value.trim();
        } catch (RuntimeException e) {
            return "";
        }
    }
}
