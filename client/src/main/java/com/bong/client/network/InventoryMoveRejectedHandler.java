package com.bong.client.network;

import com.bong.client.hud.BongToast;
import com.bong.client.util.RealmLabel;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * 库存操作拒绝原因失败 toast 处理器（plan-inventory-hint-panel-v1 P1）。
 * <p>
 * 消费 type="inventory_move_rejected" 的 S2C payload（结构化字段
 * {@code reason}/{@code required_realm}/{@code slot}/{@code cap}，见
 * {@code InventoryMoveRejectedV1} @ schema/server_data.rs），组装 "天道警示：" 前缀
 * 文案后走 <b>活链路</b>：{@link BongToast#show(String, int, long, long)} 重载
 * （不经过 {@code show(NarrationState,...)} narration 管线，也不引用死代码
 * {@code BongEventAlertOverlay} / 非 network 包 {@code EventAlertHandler}）。
 * <p>
 * 境界不足（{@code realm_too_low}）时用 {@link RealmLabel#displayName(String)} 现场把
 * server 下发的英文 tag（如 {@code "Condense"}）转中文（"凝脉"），不在 server 端硬编中文，
 * 避免措辞漂移（plan §8.1 决议 #3）。
 * <p>
 * 同一 {@code reason} 500ms 内去重，防止连续拖拽同一非法目标时 toast 刷屏。
 */
public final class InventoryMoveRejectedHandler implements ServerDataHandler {

    /** 红色警示色（复用 {@code BongToast.WARNING_COLOR} 数值，该常量在 hud 包内 package-private）。 */
    static final int WARNING_COLOR = 0xFFAA55;

    /** toast 展示时长（plan §P3 视听规格）。 */
    static final long TOAST_DURATION_MILLIS = 3000L;

    /** 同一 reason 去重窗口（plan §P1 "500ms 内去重防刷屏"）。 */
    static final long DEDUPE_WINDOW_MILLIS = 500L;

    private static final String PREFIX = "天道警示：";

    /** reason → 上次展示时间戳，供 500ms 去重。static 单例（handler 无实例状态，符合既有 handler 惯例）。 */
    private static final Map<String, Long> lastShownAtMillis = new ConcurrentHashMap<>();

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String reason = readString(payload, "reason");
        if (reason == null || reason.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "inventory_move_rejected: 'reason' 字段缺失，payload 忽略"
            );
        }

        long now = System.currentTimeMillis();
        if (isDuplicateWithinWindow(reason, now)) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "inventory_move_rejected: reason=" + reason + " 在 " + DEDUPE_WINDOW_MILLIS + "ms 去重窗口内重复，跳过"
            );
        }

        String requiredRealm = readString(payload, "required_realm");
        String slot = readString(payload, "slot");
        Integer cap = readInt(payload, "cap");

        String message = messageFor(reason, requiredRealm, slot, cap);
        recordShown(reason, now);
        BongToast.show(message, WARNING_COLOR, now, TOAST_DURATION_MILLIS);

        return ServerDataDispatch.handled(
            envelope.type(),
            "inventory_move_rejected: reason=" + reason + " toast=\"" + message + "\""
        );
    }

    /**
     * 500ms 去重窗口判定（package-private 纯函数，供测试用合成时间戳断言边界，
     * 不依赖 {@code Thread.sleep}）。
     */
    static boolean isDuplicateWithinWindow(String reason, long nowMillis) {
        Long lastShown = lastShownAtMillis.get(reason);
        return lastShown != null && (nowMillis - lastShown) < DEDUPE_WINDOW_MILLIS;
    }

    /** 记录某 reason 的展示时间戳（package-private 供测试直接摆放合成时间线）。 */
    static void recordShown(String reason, long nowMillis) {
        lastShownAtMillis.put(reason, nowMillis);
    }

    /** 清空去重 map，供测试隔离用例。 */
    static void resetForTests() {
        lastShownAtMillis.clear();
    }

    /**
     * 全部 24 个 {@code InventoryMoveRejectReason} wire tag 的文案表（package-private 供测试直接断言）。
     * 未知 reason 兜底显示通用拒绝文案（不静默丢弃 —— 拒绝已真实发生，玩家仍需被告知）。
     */
    static String messageFor(String reason, String requiredRealm, String slot, Integer cap) {
        String body = switch (reason) {
            case "realm_too_low" -> "境界不足，需达「"
                + RealmLabel.displayName(requiredRealm == null ? "" : requiredRealm) + "」方可装备此物";
            case "worn_cap_full" -> capFullMessage(slot, cap);
            case "hand_occupied" -> "该手已持械，请先卸下再更换";
            case "two_handed_locks_other" -> "双手兵器占用双手，另一侧已被锁定";
            case "equip_category_mismatch" -> "此物品类不符，无法装入该槽位";
            case "off_hand_type_mismatch" -> "副手仅可持匕首或空手武技，此物不可入副手";
            case "armor_durability_zero" -> "此甲损毁殆尽，无法穿戴";
            case "armor_slot_mismatch" -> "此甲不合此槽位" + slotSuffix(slot);
            case "armor_slot_unresolvable" -> "此物无法识别装备部位，或非制式护甲";
            case "pack_equip_slot_mismatch" -> "此背包不合此槽位" + slotSuffix(slot);
            case "worn_stack_not_top" -> "该件被上层压住，请先卸下最外层（穿戴栈仅栈顶可卸）";
            case "forbidden_in_hotbar" -> "此品类不可放入快捷栏，须留在装备槽";
            case "pack_detached" -> "背包已不在身上，无法放入内含物，请重新拾取该背包";
            case "held_worn_mismatch" -> "手槽/身体槽装备状态不符";
            case "target_out_of_bounds" -> "目标位置越界";
            case "target_occupied" -> "目标位置已被占用";
            case "multi_overlap_not_supported" -> "目标区域与多件物品重叠，无法放入";
            case "hotbar_index_out_of_range" -> "快捷栏下标越界";
            case "hotbar_occupied" -> "快捷栏该格已被占用";
            case "swap_footprint_mismatch" -> "交换失败：占用物形状与拖拽物不一致";
            case "unknown_item_template" -> "未知物品，无法操作";
            case "unknown_container_id" -> "未知容器，请重试";
            case "instance_not_found" -> "该物品已不在背包中，请重试";
            case "from_location_mismatch" -> "起始位置已变化，请重试";
            default -> "此操作被天道拒绝";
        };
        return PREFIX + body;
    }

    private static String capFullMessage(String slot, Integer cap) {
        String slotZh = slotDisplayName(slot);
        String capText = cap == null ? "?" : String.valueOf(cap);
        return slotZh + "已穿戴 " + capText + " 层，无法再叠加";
    }

    private static String slotSuffix(String slot) {
        String zh = slotDisplayName(slot);
        return zh.isEmpty() ? "" : "（应装于" + zh + "）";
    }

    /** 槽位 key → 中文，供测试直接断言。未知 slot 原样返回。 */
    static String slotDisplayName(String slot) {
        if (slot == null || slot.isBlank()) {
            return "";
        }
        return switch (slot) {
            case "head" -> "头部";
            case "chest" -> "胸部";
            case "legs" -> "腿部";
            case "feet" -> "足部";
            case "main_hand" -> "主手";
            case "off_hand" -> "副手";
            case "extra_hand_0", "extra_hand_1" -> "余手";
            default -> slot;
        };
    }

    private static String readString(JsonObject object, String fieldName) {
        JsonElement el = object.get(fieldName);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive prim = el.getAsJsonPrimitive();
        return prim.isString() ? prim.getAsString() : null;
    }

    private static Integer readInt(JsonObject object, String fieldName) {
        JsonElement el = object.get(fieldName);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive prim = el.getAsJsonPrimitive();
        if (!prim.isNumber()) {
            return null;
        }
        return prim.getAsInt();
    }
}
