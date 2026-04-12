package com.bong.client.network;

import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.state.MeridianStateStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.EnumMap;

/**
 * 解析服务端 {@code cultivation_detail} CustomPayload，翻译为 {@link MeridianBody}
 * 并推入 {@link MeridianStateStore}。
 *
 * <p>Payload 使用 SoA (parallel arrays) 布局，数组下标 0..11 对应 12 正经
 * （{@code LU, LI, ST, SP, HT, SI, BL, KI, PC, TE, GB, LR}），12..19 对应 8 奇经
 * （{@code REN, DU, CHONG, DAI, YIN_QIAO, YANG_QIAO, YIN_WEI, YANG_WEI}）；
 * 顺序与 Rust {@code MeridianId} 判别式一致（详见 server/src/cultivation/components.rs）。
 *
 * <p>与现有 PlayerStateHandler 等不同，本 handler 采用「副作用 + 返回 handled(no op payload)」
 * 模式：直接调用 {@link MeridianStateStore#replace}，避免在 {@link ServerDataDispatch}
 * 上新增 13-th 字段。Meridian snapshot 不参与 dispatch 合成与 UI 事件路由。
 */
public final class CultivationDetailHandler implements ServerDataHandler {

    /** payload 数组下标 → UI 侧 {@link MeridianChannel}（顺序与服务端 MeridianId 判别式一致）。 */
    static final MeridianChannel[] CHANNEL_ORDER = new MeridianChannel[] {
        // 12 正经: Lung, LargeIntestine, Stomach, Spleen, Heart, SmallIntestine,
        //          Bladder, Kidney, Pericardium, TripleEnergizer, Gallbladder, Liver
        MeridianChannel.LU, MeridianChannel.LI, MeridianChannel.ST, MeridianChannel.SP,
        MeridianChannel.HT, MeridianChannel.SI, MeridianChannel.BL, MeridianChannel.KI,
        MeridianChannel.PC, MeridianChannel.TE, MeridianChannel.GB, MeridianChannel.LR,
        // 8 奇经: Ren, Du, Chong, Dai, YinQiao, YangQiao, YinWei, YangWei
        MeridianChannel.REN, MeridianChannel.DU, MeridianChannel.CHONG, MeridianChannel.DAI,
        MeridianChannel.YIN_QIAO, MeridianChannel.YANG_QIAO, MeridianChannel.YIN_WEI, MeridianChannel.YANG_WEI
    };

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();

        JsonArray opened = readArray(payload, "opened");
        JsonArray flowRate = readArray(payload, "flow_rate");
        JsonArray flowCapacity = readArray(payload, "flow_capacity");
        JsonArray integrity = readArray(payload, "integrity");

        if (opened == null || flowRate == null || flowCapacity == null || integrity == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring cultivation_detail payload: missing required array field(s)"
            );
        }
        int expected = CHANNEL_ORDER.length;
        if (opened.size() != expected || flowRate.size() != expected
            || flowCapacity.size() != expected || integrity.size() != expected) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring cultivation_detail payload: array length mismatch (expected " + expected + ")"
            );
        }

        // 可选扩展字段：realm / open_progress / cracks_count / contamination_total。
        // 数组长度不合法时忽略；不报错，保持向前兼容。
        JsonArray openProgress = readArray(payload, "open_progress");
        if (openProgress != null && openProgress.size() != expected) openProgress = null;
        JsonArray cracksCount = readArray(payload, "cracks_count");
        if (cracksCount != null && cracksCount.size() != expected) cracksCount = null;
        String realm = readString(payload, "realm");
        double contaminationTotal = readDouble(payload, "contamination_total");

        MeridianBody body = buildBody(opened, flowRate, flowCapacity, integrity, openProgress, cracksCount, realm, contaminationTotal);
        MeridianStateStore.replace(body);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied cultivation_detail snapshot (20 channels) to MeridianStateStore"
        );
    }

    static MeridianBody buildBody(JsonArray opened, JsonArray flowRate, JsonArray flowCapacity, JsonArray integrity) {
        return buildBody(opened, flowRate, flowCapacity, integrity, null, null, null, 0.0);
    }

    static MeridianBody buildBody(JsonArray opened, JsonArray flowRate, JsonArray flowCapacity, JsonArray integrity,
                                  JsonArray openProgress, JsonArray cracksCount, String realm,
                                  double contaminationTotal) {
        EnumMap<MeridianChannel, ChannelState> channels = new EnumMap<>(MeridianChannel.class);
        for (int i = 0; i < CHANNEL_ORDER.length; i++) {
            MeridianChannel ch = CHANNEL_ORDER[i];
            boolean isOpened = asBool(opened.get(i));
            double capacity = asDouble(flowCapacity.get(i));
            double rate = asDouble(flowRate.get(i));
            double integ = clamp01(asDouble(integrity.get(i)));
            ChannelState.DamageLevel dmg = damageFromIntegrity(integ);
            // 未打通经脉：open_progress ∈ [0,1] 复用为 healProgress（UI 里作"打通进度条"）。
            double healProgress = 0.0;
            if (!isOpened && openProgress != null) {
                healProgress = clamp01(asDouble(openProgress.get(i)));
            }
            channels.put(ch, new ChannelState(
                ch,
                capacity,
                Math.min(rate, capacity),
                dmg,
                /* contamination */ 0.0,
                healProgress,
                /* blocked       */ !isOpened
            ));
        }
        MeridianBody.Builder builder = MeridianBody.builder().channels(channels);
        if (realm != null && !realm.isEmpty()) {
            builder.realm(realm);
        }
        if (cracksCount != null) {
            java.util.EnumMap<MeridianChannel, Integer> map = new java.util.EnumMap<>(MeridianChannel.class);
            for (int i = 0; i < CHANNEL_ORDER.length; i++) {
                int n = (int) Math.max(0, asDouble(cracksCount.get(i)));
                if (n > 0) map.put(CHANNEL_ORDER[i], n);
            }
            builder.cracksCount(map);
        }
        builder.contaminationTotal(Math.max(0.0, contaminationTotal));
        return builder.build();
    }

    /** 将 integrity∈[0,1] 离散成 UI 可识别的 {@link ChannelState.DamageLevel}。 */
    static ChannelState.DamageLevel damageFromIntegrity(double integ) {
        if (integ >= 0.95) return ChannelState.DamageLevel.INTACT;
        if (integ >= 0.70) return ChannelState.DamageLevel.MICRO_TEAR;
        if (integ >= 0.10) return ChannelState.DamageLevel.TORN;
        return ChannelState.DamageLevel.SEVERED;
    }

    private static JsonArray readArray(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonArray()) ? el.getAsJsonArray() : null;
    }

    private static double readDouble(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0.0;
        double v = el.getAsDouble();
        return Double.isFinite(v) ? v : 0.0;
    }

    private static String readString(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isString())
            ? el.getAsString() : null;
    }

    private static boolean asBool(JsonElement el) {
        return el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isBoolean()
            && el.getAsBoolean();
    }

    private static double asDouble(JsonElement el) {
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0.0;
        double v = el.getAsDouble();
        return Double.isFinite(v) ? v : 0.0;
    }

    private static double clamp01(double v) {
        return Math.max(0.0, Math.min(1.0, v));
    }
}
