package com.bong.client.network;

import com.bong.client.cultivation.ColorKind;
import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.state.BodyPlanLayoutStore;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillMilestoneSnapshot;
import com.bong.client.skill.SkillMilestoneStore;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;

/**
 * 解析服务端 {@code cultivation_detail} CustomPayload，翻译为 {@link MeridianBody}
 * 并推入 {@link MeridianStateStore}。
 *
 * <p>Payload 使用 SoA (parallel arrays) 布局。plan-race-system-v1 P1c 起，数组下标不再
 * 隐式对应固定的 20 条 TCM 经脉顺序——server 随数组附带 {@code channel_ids}（snake_case
 * channel id 字符串，与 opened/flow_rate/... 等数组同序同长），本 handler 按
 * {@code channel_ids[i]} keyed 查找对应 {@link MeridianChannel}（{@link
 * MeridianChannel#fromChannelId}），而非假设固定位置数组。本段只保证 humanoid 20 条
 * channel id 渲染不回归；非 humanoid channel（P5 飞鲸等）目前没有 UI 展示位，
 * {@code fromChannelId} 返回 {@code null} 时静默跳过该下标（不 crash，不伪造）——
 * 布局数据驱动留给 P2。
 */
public final class CultivationDetailHandler implements ServerDataHandler {

    /**
     * 历史遗留：payload 缺失 {@code channel_ids}（理论上不该发生，server 侧
     * {@code cultivation_detail_emit.rs} 恒发）时的 fallback 顺序，仅用于诊断日志，
     * 不再作为解码路径的权威真源。
     */
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
        int expected = opened.size();
        if (flowRate.size() != expected || flowCapacity.size() != expected || integrity.size() != expected) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring cultivation_detail payload: array length mismatch (opened=" + expected + ")"
            );
        }

        // plan-race-system-v1 P1c：channel_ids 是 keyed 解码的权威真源，与
        // opened/flow_rate/... 同序同长；缺失或长度不符时退回 legacy 固定位置顺序
        // （仅诊断兜底，humanoid 20 条恰好长度相符时才生效，见类头文档）。
        MeridianChannel[] channelOrder = resolveChannelOrder(readArray(payload, "channel_ids"), expected);

        // 可选扩展字段：realm / open_progress / cracks_count / contamination_total。
        // 数组长度不合法时忽略；不报错，保持向前兼容。
        JsonArray openProgress = readArray(payload, "open_progress");
        if (openProgress != null && openProgress.size() != expected) openProgress = null;
        JsonArray cracksCount = readArray(payload, "cracks_count");
        if (cracksCount != null && cracksCount.size() != expected) cracksCount = null;
        String realm = readString(payload, "realm");
        double contaminationTotal = readDouble(payload, "contamination_total");
        JsonObject lifespan = readObject(payload, "lifespan");
        ColorKind qiColorMain = ColorKind.fromWire(readString(payload, "qi_color_main"));
        ColorKind qiColorSecondary = ColorKind.fromWire(readString(payload, "qi_color_secondary"));
        boolean qiColorChaotic = readBoolean(payload, "qi_color_chaotic");
        boolean qiColorHunyuan = readBoolean(payload, "qi_color_hunyuan");
        EnumMap<ColorKind, Double> practiceWeights = parsePracticeWeights(readArray(payload, "practice_weights"));
        // plan-race-system-v1 P1c：target_meridian 现为 channel id 字符串（不再是数组
        // 下标），直接经 MeridianChannel.fromChannelId 查找；非 humanoid channel（无
        // 对应 UI 枚举）合法地解析为 null，不 crash。
        MeridianChannel targetMeridian = MeridianChannel.fromChannelId(readString(payload, "target_meridian"));

        // plan-race-system-v1 P2b — cultivation_detail 附带的本体 body_plan_id 是
        // BodyPlanLayoutStore"当前"指针的唯一权威来源；body_plan_layout payload 本身
        // 只按 id 建缓存（见 BodyPlanLayoutHandler），两者到达顺序不定，store 内部处理竞态。
        BodyPlanLayoutStore.setCurrentPlanId(readString(payload, "body_plan_id"));

        MeridianBody body = buildBody(
            channelOrder,
            opened,
            flowRate,
            flowCapacity,
            integrity,
            openProgress,
            cracksCount,
            realm,
            contaminationTotal,
            lifespan,
            qiColorMain,
            qiColorSecondary,
            qiColorChaotic,
            qiColorHunyuan,
            practiceWeights,
            targetMeridian
        );
        MeridianStateStore.replace(body);
        syncSkillCapsFromRealm(realm);
        SkillMilestoneStore.replace(
            parseSkillMilestones(payload.getAsJsonArray("skill_milestones")),
            readString(payload, "recent_skill_milestones_summary")
        );
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied cultivation_detail snapshot (" + expected + " channels, keyed by channel_ids) to MeridianStateStore"
        );
    }

    /**
     * 按 {@code channel_ids[i]} keyed 查找 {@link MeridianChannel}；查不到的下标（非
     * humanoid channel）落 {@code null}，调用方需静默跳过而非 crash。{@code channel_ids}
     * 缺失或长度不符 {@code expected} 时，仅当 {@code expected == CHANNEL_ORDER.length}
     * （humanoid 20 条）才退回 legacy 固定顺序兜底，否则整段落 null（未知构型不渲染）。
     */
    static MeridianChannel[] resolveChannelOrder(JsonArray channelIds, int expected) {
        MeridianChannel[] resolved = new MeridianChannel[expected];
        if (channelIds != null && channelIds.size() == expected) {
            for (int i = 0; i < expected; i++) {
                JsonElement el = channelIds.get(i);
                String id = (el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isString())
                    ? el.getAsString() : null;
                resolved[i] = MeridianChannel.fromChannelId(id);
            }
            return resolved;
        }
        if (expected == CHANNEL_ORDER.length) {
            System.arraycopy(CHANNEL_ORDER, 0, resolved, 0, expected);
        }
        return resolved;
    }

    static MeridianBody buildBody(JsonArray opened, JsonArray flowRate, JsonArray flowCapacity, JsonArray integrity) {
        return buildBody(CHANNEL_ORDER, opened, flowRate, flowCapacity, integrity, null, null, null, 0.0, null);
    }

    static MeridianBody buildBody(MeridianChannel[] channelOrder,
                                   JsonArray opened, JsonArray flowRate, JsonArray flowCapacity, JsonArray integrity,
                                   JsonArray openProgress, JsonArray cracksCount, String realm,
                                   double contaminationTotal, JsonObject lifespan) {
        return buildBody(channelOrder, opened, flowRate, flowCapacity, integrity, openProgress, cracksCount, realm,
            contaminationTotal, lifespan, null, null, false, false, new EnumMap<>(ColorKind.class));
    }

    static MeridianBody buildBody(MeridianChannel[] channelOrder,
                                   JsonArray opened, JsonArray flowRate, JsonArray flowCapacity, JsonArray integrity,
                                   JsonArray openProgress, JsonArray cracksCount, String realm,
                                   double contaminationTotal, JsonObject lifespan,
                                   ColorKind qiColorMain, ColorKind qiColorSecondary,
                                   boolean qiColorChaotic, boolean qiColorHunyuan,
                                   EnumMap<ColorKind, Double> practiceWeights) {
        return buildBody(channelOrder, opened, flowRate, flowCapacity, integrity, openProgress, cracksCount, realm,
            contaminationTotal, lifespan, qiColorMain, qiColorSecondary, qiColorChaotic, qiColorHunyuan,
            practiceWeights, null);
    }

    /**
     * plan-race-system-v1 P1c — {@code channelOrder[i]} 是 {@code opened}/{@code
     * flowRate}/... 等并行数组第 i 项对应的 {@link MeridianChannel}（由调用方按
     * {@code channel_ids} keyed 解析出，见 {@link #resolveChannelOrder}）；{@code null}
     * 项（未知 / 非 humanoid channel）静默跳过，不参与 {@link MeridianBody} 渲染。
     */
    static MeridianBody buildBody(MeridianChannel[] channelOrder,
                                   JsonArray opened, JsonArray flowRate, JsonArray flowCapacity, JsonArray integrity,
                                   JsonArray openProgress, JsonArray cracksCount, String realm,
                                   double contaminationTotal, JsonObject lifespan,
                                   ColorKind qiColorMain, ColorKind qiColorSecondary,
                                   boolean qiColorChaotic, boolean qiColorHunyuan,
                                   EnumMap<ColorKind, Double> practiceWeights,
                                   MeridianChannel targetMeridian) {
        EnumMap<MeridianChannel, ChannelState> channels = new EnumMap<>(MeridianChannel.class);
        for (int i = 0; i < channelOrder.length; i++) {
            MeridianChannel ch = channelOrder[i];
            if (ch == null) continue;
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
            for (int i = 0; i < channelOrder.length; i++) {
                MeridianChannel ch = channelOrder[i];
                if (ch == null) continue;
                int n = (int) Math.max(0, asDouble(cracksCount.get(i)));
                if (n > 0) map.put(ch, n);
            }
            builder.cracksCount(map);
        }
        builder.contaminationTotal(Math.max(0.0, contaminationTotal));
        if (lifespan != null) {
            builder.lifespanPreview(
                readDouble(lifespan, "years_lived"),
                (int) Math.max(0, readDouble(lifespan, "cap_by_realm")),
                readDouble(lifespan, "remaining_years"),
                (int) Math.max(0, readDouble(lifespan, "death_penalty_years")),
                readDouble(lifespan, "tick_rate_multiplier"),
                readBoolean(lifespan, "is_wind_candle")
            );
        }
        builder.qiColor(qiColorMain, qiColorSecondary, qiColorChaotic, qiColorHunyuan);
        builder.qiColorPracticeWeights(practiceWeights);
        builder.targetMeridian(targetMeridian);
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

    private static JsonObject readObject(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return (el != null && el.isJsonObject()) ? el.getAsJsonObject() : null;
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

    private static boolean readBoolean(JsonObject obj, String name) {
        JsonElement el = obj.get(name);
        return el != null && el.isJsonPrimitive() && el.getAsJsonPrimitive().isBoolean()
            && el.getAsBoolean();
    }

    private static EnumMap<ColorKind, Double> parsePracticeWeights(JsonArray weights) {
        EnumMap<ColorKind, Double> out = new EnumMap<>(ColorKind.class);
        if (weights == null) return out;
        for (JsonElement element : weights) {
            if (element == null || !element.isJsonObject()) continue;
            JsonObject obj = element.getAsJsonObject();
            ColorKind color = ColorKind.fromWire(readString(obj, "color"));
            double weight = readDouble(obj, "weight");
            if (color != null && weight > 0.0) {
                out.put(color, weight);
            }
        }
        return out;
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

    static void syncSkillCapsFromRealm(String realm) {
        Integer cap = skillCapForRealm(realm);
        if (cap == null) return;

        SkillSetSnapshot snapshot = SkillSetStore.snapshot();
        for (SkillId skill : SkillId.values()) {
            SkillSetSnapshot.Entry cur = snapshot.get(skill);
            SkillSetStore.updateEntry(
                skill,
                new SkillSetSnapshot.Entry(
                    cur.lv(),
                    cur.xp(),
                    cur.xpToNext(),
                    cur.totalXp(),
                    cap,
                    cur.recentGainXp(),
                    cur.recentGainMillis()
                )
            );
        }
    }

    static Integer skillCapForRealm(String realm) {
        if (realm == null || realm.isEmpty()) return null;
        return switch (realm) {
            case "Awaken" -> 3;
            case "Induce" -> 5;
            case "Condense" -> 7;
            case "Solidify" -> 8;
            case "Spirit" -> 9;
            case "Void" -> 10;
            default -> null;
        };
    }

    static List<SkillMilestoneSnapshot> parseSkillMilestones(JsonArray milestones) {
        if (milestones == null) return List.of();
        ArrayList<SkillMilestoneSnapshot> out = new ArrayList<>();
        for (JsonElement element : milestones) {
            if (element == null || !element.isJsonObject()) continue;
            JsonObject obj = element.getAsJsonObject();
            SkillId skill = SkillId.fromWire(readString(obj, "skill"));
            if (skill == null) continue;
            out.add(new SkillMilestoneSnapshot(
                skill,
                (int) readDouble(obj, "new_lv"),
                (long) readDouble(obj, "achieved_at"),
                readString(obj, "narration"),
                (long) readDouble(obj, "total_xp_at")
            ));
        }
        return List.copyOf(out);
    }
}
