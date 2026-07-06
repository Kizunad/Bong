package com.bong.client.spider;

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
 * plan-fauna-mimic-spider-v1 P2 — 拟态灰烬蛛伪装渲染处理器。
 *
 * <p>处理两个 CustomPayload channel：
 * <ul>
 *   <li>{@code bong:spider_disguise_enter}：默认接收当前所有 Disguised 蛛的 MC entity_id
 *       列表并全量替换；当 {@code full_sync=false} 时按增量加入。
 *   <li>{@code bong:spider_ambush_trigger}：蛛暴起时，从列表移除对应 entity_id，
 *       client 恢复正常蜘蛛渲染，并记录为 revealed，供旧 metadata 窗口恢复名牌。
 * </ul>
 *
 * <p>wire payload 格式（两 channel 共用，仅 {@code type} 字段区分）：
 * <pre>{@code
 * {
 *   "v": 1,
 *   "type": "spider_disguise_enter" | "spider_ambush_trigger",
 *   "entity_ids": [42, 77, ...],
 *   "full_sync": false // 可选，仅 spider_disguise_enter 增量同步使用
 * }
 * }</pre>
 *
 * <p>线程安全：{@link #DISGUISED_ENTITY_IDS} 使用 {@link CopyOnWriteArraySet}，
 * 网络线程和渲染线程可安全并发读写（渲染线程通过 {@link #isDisguised} 查询）。
 */
public final class SpiderDisguiseHandler {

    /** bong:spider_disguise_enter channel identifier components. */
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH_ENTER = "spider_disguise_enter";
    public static final String CHANNEL_PATH_AMBUSH = "spider_ambush_trigger";

    /**
     * 当前处于 Disguised（ash_block 外观）状态的蛛的 MC entity id 集合。
     * 渲染层通过 {@link #isDisguised(int)} 查询是否需要切换贴图。
     */
    private static final Set<Integer> DISGUISED_ENTITY_IDS = new CopyOnWriteArraySet<>();
    private static final Set<Integer> REVEALED_ENTITY_IDS = new CopyOnWriteArraySet<>();

    private SpiderDisguiseHandler() {
    }

    /**
     * 处理 {@code bong:spider_disguise_enter} payload。
     *
     * <p>将 payload 中所有 entity_id 加入 {@link #DISGUISED_ENTITY_IDS}。
     *
     * @return {@code true} 表示 payload 有效且已处理，{@code false} 表示格式错误
     */
    public static boolean handleEnter(String jsonPayload, int payloadSizeBytes) {
        return handle(jsonPayload, payloadSizeBytes, "spider_disguise_enter", true);
    }

    /**
     * 处理 {@code bong:spider_ambush_trigger} payload。
     *
     * <p>将 payload 中所有 entity_id 从 {@link #DISGUISED_ENTITY_IDS} 移除。
     * 蛛暴起后渲染恢复正常蜘蛛外观。
     *
     * @return {@code true} 表示 payload 有效且已处理，{@code false} 表示格式错误
     */
    public static boolean handleAmbush(String jsonPayload, int payloadSizeBytes) {
        return handle(jsonPayload, payloadSizeBytes, "spider_ambush_trigger", false);
    }

    /**
     * 查询指定 MC entity id 的蛛是否处于 Disguised（ash_block）渲染状态。
     *
     * @param entityId MC 协议 entity id（int）
     * @return {@code true} 表示当前应渲染为灰烬方块
     */
    public static boolean isDisguised(int entityId) {
        return DISGUISED_ENTITY_IDS.contains(entityId);
    }

    /**
     * 查询指定 MC entity id 是否已收到过 ambush 暴露事件。
     *
     * <p>该状态用于覆盖旧的 {@code nametag_visible=false} metadata：蛛已经暴起时，
     * 即使下一帧 metadata 还没刷新，也不应继续按伪装期隐藏自定义名牌。
     */
    public static boolean isRevealed(int entityId) {
        return REVEALED_ENTITY_IDS.contains(entityId);
    }

    /**
     * 返回当前处于 Disguised 状态的所有 entity id 的只读快照（测试用）。
     */
    public static List<Integer> disguisedEntityIdsSnapshot() {
        return Collections.unmodifiableList(new ArrayList<>(DISGUISED_ENTITY_IDS));
    }

    public static List<Integer> revealedEntityIdsSnapshot() {
        return Collections.unmodifiableList(new ArrayList<>(REVEALED_ENTITY_IDS));
    }

    /**
     * 断线时清空状态，防止跨 session 状态泄漏。
     */
    public static void clearOnDisconnect() {
        DISGUISED_ENTITY_IDS.clear();
        REVEALED_ENTITY_IDS.clear();
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
            boolean fullSync = booleanField(root, "full_sync", true);
            if (fullSync) {
                // 周期/进服 full sync：服务端给出完整 Disguised 列表。
                DISGUISED_ENTITY_IDS.clear();
            }
            DISGUISED_ENTITY_IDS.addAll(ids);
            REVEALED_ENTITY_IDS.removeAll(ids);
        } else {
            // spider_ambush_trigger：只移除触发暴起的蛛（增量）
            DISGUISED_ENTITY_IDS.removeAll(ids);
            REVEALED_ENTITY_IDS.addAll(ids);
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

    private static boolean booleanField(JsonObject root, String fieldName, boolean fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            return root.get(fieldName).getAsBoolean();
        } catch (RuntimeException e) {
            return fallback;
        }
    }
}
