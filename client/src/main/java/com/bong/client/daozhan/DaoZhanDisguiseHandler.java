package com.bong.client.daozhan;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Set;
import java.util.concurrent.CopyOnWriteArraySet;

/**
 * plan-daozhan-v1 P1 — 道伥伪装渲染处理器。
 *
 * <p>处理两个 CustomPayload channel：
 * <ul>
 *   <li>{@code bong:daozhan_disguise_enter}：接收当前所有 Mimicry 态道伥的 MC entity_id 列表，
 *       将其记录到 {@link #DISGUISED_ENTITY_IDS}，供渲染层（{@link FakePlayerRendererMixin}）
 *       将对应实体切换为无名玩家（FakePlayerEntity）外观。
 *   <li>{@code bong:daozhan_reveal}：道伥从 Mimicry 暴起时，从列表移除对应 entity_id，
 *       client 恢复正常道伥（Daoxiang）渲染。
 * </ul>
 *
 * <p>wire payload 格式（两 channel 共用，仅 {@code type} 字段区分）：
 * <pre>{@code
 * {
 *   "v": 1,
 *   "type": "daozhan_disguise_enter" | "daozhan_reveal",
 *   "entity_ids": [42, 77, ...]
 * }
 * }</pre>
 *
 * <p>设计约束：
 * <ul>
 *   <li>禁止 vanilla MC entity hack（不用 armor stand / invisible player mob 充当伪装体）。
 *   <li>走 CustomPayload + client 自渲染 FakePlayerEntity，同 spider_disguise 既有模式。
 *   <li>计时用游戏 tick（{@code ClientTickEvents}），非渲染帧（fauna-stitched 教训）。
 * </ul>
 *
 * <p>线程安全：{@link #DISGUISED_ENTITY_IDS} 使用 {@link CopyOnWriteArraySet}，
 * 网络线程和渲染线程可安全并发读写（渲染层通过 {@link #isDisguised} 查询）。
 */
public final class DaoZhanDisguiseHandler {

    /** bong:daozhan_disguise_enter channel identifier components. */
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH_ENTER = "daozhan_disguise_enter";
    public static final String CHANNEL_PATH_REVEAL = "daozhan_reveal";

    /**
     * 当前处于 Mimicry（FakePlayer 外观）状态的道伥的 MC entity id 集合。
     * 渲染层通过 {@link #isDisguised(int)} 查询是否需要切换为玩家模型渲染。
     */
    private static final Set<Integer> DISGUISED_ENTITY_IDS = new CopyOnWriteArraySet<>();

    private DaoZhanDisguiseHandler() {
    }

    /**
     * 处理 {@code bong:daozhan_disguise_enter} payload。
     *
     * <p>全量替换 {@link #DISGUISED_ENTITY_IDS}（先清空再加入），确保 client 与 server 完全同步。
     *
     * @return {@code true} 表示 payload 有效且已处理，{@code false} 表示格式错误
     */
    public static boolean handleEnter(String jsonPayload, int payloadSizeBytes) {
        return handle(jsonPayload, payloadSizeBytes, "daozhan_disguise_enter", true);
    }

    /**
     * 处理 {@code bong:daozhan_reveal} payload。
     *
     * <p>将 payload 中所有 entity_id 从 {@link #DISGUISED_ENTITY_IDS} 移除。
     * 道伥暴起后渲染恢复正常 Daoxiang 外观。
     *
     * @return {@code true} 表示 payload 有效且已处理，{@code false} 表示格式错误
     */
    public static boolean handleReveal(String jsonPayload, int payloadSizeBytes) {
        return handle(jsonPayload, payloadSizeBytes, "daozhan_reveal", false);
    }

    /**
     * 查询指定 MC entity id 的道伥是否处于 Mimicry（FakePlayer）渲染状态。
     *
     * <p>渲染层（{@link FakePlayerRendererMixin}）调用此方法判断是否注入玩家模型渲染覆盖。
     *
     * @param entityId MC 协议 entity id（int）
     * @return {@code true} 表示当前应渲染为无名玩家
     */
    public static boolean isDisguised(int entityId) {
        return DISGUISED_ENTITY_IDS.contains(entityId);
    }

    /**
     * 返回当前处于 Mimicry 状态的所有 entity id 的只读快照（测试用）。
     */
    public static List<Integer> disguisedEntityIdsSnapshot() {
        return Collections.unmodifiableList(new ArrayList<>(DISGUISED_ENTITY_IDS));
    }

    /**
     * 断线时清空状态，防止跨 session 状态泄漏。
     */
    public static void clearOnDisconnect() {
        DISGUISED_ENTITY_IDS.clear();
    }

    // ── 内部 ─────────────────────────────────────────────────────────────────

    private static boolean handle(String jsonPayload, int payloadSizeBytes, String expectedType, boolean add) {
        if (jsonPayload == null || payloadSizeBytes < 0 || payloadSizeBytes > ServerDataEnvelope.MAX_PAYLOAD_BYTES) {
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
        String type = stringField(root, "type", "");
        if (!expectedType.equals(type)) {
            return false;
        }

        List<Integer> ids = parseEntityIds(root);
        if (add) {
            // daozhan_disguise_enter：全量替换（server 周期性全量 sync）
            DISGUISED_ENTITY_IDS.clear();
            DISGUISED_ENTITY_IDS.addAll(ids);
        } else {
            // daozhan_reveal：只移除暴起的道伥（增量移除）
            DISGUISED_ENTITY_IDS.removeAll(ids);
        }
        return true;
    }

    private static List<Integer> parseEntityIds(JsonObject root) {
        if (!root.has("entity_ids") || root.get("entity_ids").isJsonNull()) {
            return List.of();
        }
        JsonElement element = root.get("entity_ids");
        if (!element.isJsonArray()) {
            return List.of();
        }
        JsonArray array = element.getAsJsonArray();
        List<Integer> result = new ArrayList<>(array.size());
        for (JsonElement item : array) {
            if (item == null || item.isJsonNull()) {
                continue;
            }
            try {
                result.add(item.getAsInt());
            } catch (RuntimeException ignored) {
                // 跳过格式错误的 id
            }
        }
        return result;
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

    private static String stringField(JsonObject root, String fieldName, String fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            String value = root.get(fieldName).getAsString();
            return value == null || value.isBlank() ? fallback : value.trim();
        } catch (RuntimeException e) {
            return fallback;
        }
    }
}
