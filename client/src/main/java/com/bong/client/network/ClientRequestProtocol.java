package com.bong.client.network;

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
