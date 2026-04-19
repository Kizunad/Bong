package com.bong.client.network;

import com.bong.client.botany.BotanyHarvestMode;
import com.bong.client.inventory.model.MeridianChannel;
import com.google.gson.JsonObject;

/**
 * 客户端 → 服务端 {@code bong:client_request} 通道的协议常量与 JSON 编码。
 *
 * <p>与 Rust {@code server/src/schema/client_request.rs} 和 TypeScript
 * {@code agent/packages/schema/src/client-request.ts} 1:1 对齐。</p>
 *
 * <p>消息形状：{@code {"type": "<snake_case>", "v": 1, ...}}。</p>
 */
public final class ClientRequestProtocol {
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "client_request";
    public static final int VERSION = 1;

    /** 服务端 {@code MeridianId} 的 PascalCase 字面量（serde 默认序列化）。 */
    public enum MeridianId {
        // 12 正经
        Lung, LargeIntestine, Stomach, Spleen, Heart, SmallIntestine,
        Bladder, Kidney, Pericardium, TripleEnergizer, Gallbladder, Liver,
        // 8 奇经
        Ren, Du, Chong, Dai, YinQiao, YangQiao, YinWei, YangWei
    }

    /** 服务端 {@code ForgeAxis}（serde 默认 PascalCase）。 */
    public enum ForgeAxis { Rate, Capacity }

    private ClientRequestProtocol() {}

    /** 将 UI 侧 {@link MeridianChannel} 枚举映射为服务端 {@link MeridianId}。 */
    public static MeridianId toMeridianId(MeridianChannel ch) {
        return switch (ch) {
            case LU -> MeridianId.Lung;
            case LI -> MeridianId.LargeIntestine;
            case ST -> MeridianId.Stomach;
            case SP -> MeridianId.Spleen;
            case HT -> MeridianId.Heart;
            case SI -> MeridianId.SmallIntestine;
            case BL -> MeridianId.Bladder;
            case KI -> MeridianId.Kidney;
            case PC -> MeridianId.Pericardium;
            case TE -> MeridianId.TripleEnergizer;
            case GB -> MeridianId.Gallbladder;
            case LR -> MeridianId.Liver;
            case REN -> MeridianId.Ren;
            case DU -> MeridianId.Du;
            case CHONG -> MeridianId.Chong;
            case DAI -> MeridianId.Dai;
            case YIN_QIAO -> MeridianId.YinQiao;
            case YANG_QIAO -> MeridianId.YangQiao;
            case YIN_WEI -> MeridianId.YinWei;
            case YANG_WEI -> MeridianId.YangWei;
        };
    }

    public static String encodeSetMeridianTarget(MeridianId meridian) {
        JsonObject obj = envelope("set_meridian_target");
        obj.addProperty("meridian", meridian.name());
        return obj.toString();
    }

    public static String encodeBreakthroughRequest() {
        return envelope("breakthrough_request").toString();
    }

    /**
     * 顿悟决定 C2S 回执。{@code chosenIdx = null} 表示拒绝或超时；否则为选中候选下标（0-based）。
     */
    public static String encodeInsightDecision(String triggerId, Integer chosenIdx) {
        JsonObject obj = envelope("insight_decision");
        obj.addProperty("trigger_id", triggerId);
        if (chosenIdx == null) {
            obj.add("choice_idx", com.google.gson.JsonNull.INSTANCE);
        } else {
            if (chosenIdx < 0) {
                throw new IllegalArgumentException("chosenIdx must be >= 0, got " + chosenIdx);
            }
            obj.addProperty("choice_idx", chosenIdx.intValue());
        }
        return obj.toString();
    }

    public static String encodeForgeRequest(MeridianId meridian, ForgeAxis axis) {
        JsonObject obj = envelope("forge_request");
        obj.addProperty("meridian", meridian.name());
        obj.addProperty("axis", axis.name());
        return obj.toString();
    }

    public static String encodeBotanyHarvestRequest(String sessionId, BotanyHarvestMode mode) {
        if (sessionId == null || sessionId.isBlank()) {
            throw new IllegalArgumentException("sessionId must not be blank");
        }
        if (mode == null) {
            throw new IllegalArgumentException("mode must not be null");
        }
        JsonObject obj = envelope("botany_harvest_request");
        obj.addProperty("session_id", sessionId);
        obj.addProperty("mode", mode.wireName());
        return obj.toString();
    }

    // ─── 炼丹 (plan-alchemy-v1 §4) ──────────────────────────────────────────

    public static String encodeAlchemyOpenFurnace(String furnaceId) {
        JsonObject obj = envelope("alchemy_open_furnace");
        obj.addProperty("furnace_id", furnaceId);
        return obj.toString();
    }

    public static String encodeAlchemyTurnPage(int delta) {
        JsonObject obj = envelope("alchemy_turn_page");
        obj.addProperty("delta", delta);
        return obj.toString();
    }

    public static String encodeAlchemyLearnRecipe(String recipeId) {
        JsonObject obj = envelope("alchemy_learn_recipe");
        obj.addProperty("recipe_id", recipeId);
        return obj.toString();
    }

    public static String encodeAlchemyIgnite(String recipeId) {
        JsonObject obj = envelope("alchemy_ignite");
        obj.addProperty("recipe_id", recipeId);
        return obj.toString();
    }

    public static String encodeAlchemyFeedSlot(int slotIdx, String material, int count) {
        JsonObject obj = envelope("alchemy_feed_slot");
        obj.addProperty("slot_idx", slotIdx);
        obj.addProperty("material", material);
        obj.addProperty("count", count);
        return obj.toString();
    }

    public static String encodeAlchemyTakeBack(int slotIdx) {
        JsonObject obj = envelope("alchemy_take_back");
        obj.addProperty("slot_idx", slotIdx);
        return obj.toString();
    }

    public static String encodeAlchemyInjectQi(double qi) {
        JsonObject obj = envelope("alchemy_intervention");
        JsonObject inner = new JsonObject();
        inner.addProperty("kind", "inject_qi");
        inner.addProperty("qi", qi);
        obj.add("intervention", inner);
        return obj.toString();
    }

    public static String encodeAlchemyAdjustTemp(double temp) {
        JsonObject obj = envelope("alchemy_intervention");
        JsonObject inner = new JsonObject();
        inner.addProperty("kind", "adjust_temp");
        inner.addProperty("temp", temp);
        obj.add("intervention", inner);
        return obj.toString();
    }

    public static String encodeAlchemyTakePill(String pillItemId) {
        JsonObject obj = envelope("alchemy_take_pill");
        obj.addProperty("pill_item_id", pillItemId);
        return obj.toString();
    }

    // ─── Inventory move intent (client → server) ────────────────────────────

    /** 库存位置三态联合，匹配 server schema InventoryLocationV1。 */
    public sealed interface InvLocation {
        JsonObject toJson();
    }
    public record ContainerLoc(String containerId, int row, int col) implements InvLocation {
        public JsonObject toJson() {
            JsonObject o = new JsonObject();
            o.addProperty("kind", "container");
            o.addProperty("container_id", containerId);
            o.addProperty("row", row);
            o.addProperty("col", col);
            return o;
        }
    }
    public record EquipLoc(String slot) implements InvLocation {
        public JsonObject toJson() {
            JsonObject o = new JsonObject();
            o.addProperty("kind", "equip");
            o.addProperty("slot", slot);
            return o;
        }
    }
    public record HotbarLoc(int index) implements InvLocation {
        public JsonObject toJson() {
            JsonObject o = new JsonObject();
            o.addProperty("kind", "hotbar");
            o.addProperty("index", index);
            return o;
        }
    }

    public static String encodeInventoryMove(long instanceId, InvLocation from, InvLocation to) {
        JsonObject obj = envelope("inventory_move_intent");
        obj.addProperty("instance_id", instanceId);
        obj.add("from", from.toJson());
        obj.add("to", to.toJson());
        return obj.toString();
    }

    // ─── HUD combat intents (plan-HUD-v1 §11.3) ─────────────────────────────

    public static String encodeUseQuickSlot(int slot) {
        JsonObject obj = envelope("use_quick_slot");
        obj.addProperty("slot", slot);
        return obj.toString();
    }

    /** itemId == null → 清空槽位。 */
    public static String encodeQuickSlotBind(int slot, String itemId) {
        JsonObject obj = envelope("quick_slot_bind");
        obj.addProperty("slot", slot);
        if (itemId == null || itemId.isEmpty()) {
            obj.add("item_id", com.google.gson.JsonNull.INSTANCE);
        } else {
            obj.addProperty("item_id", itemId);
        }
        return obj.toString();
    }

    public static String encodeJiemai() {
        return envelope("jiemai").toString();
    }

    public static String encodeSwitchDefenseStance(String stance) {
        JsonObject obj = envelope("switch_defense_stance");
        obj.addProperty("stance", stance);
        return obj.toString();
    }

    /** 通用请求编码（combat UI 系列使用）。payload 可为 {@code null}。 */
    public static String encodeGeneric(String type, JsonObject payload) {
        JsonObject obj = envelope(type);
        if (payload != null) {
            for (String key : payload.keySet()) {
                obj.add(key, payload.get(key));
            }
        }
        return obj.toString();
    }

    private static JsonObject envelope(String type) {
        JsonObject obj = new JsonObject();
        obj.addProperty("type", type);
        obj.addProperty("v", VERSION);
        return obj;
    }
}
