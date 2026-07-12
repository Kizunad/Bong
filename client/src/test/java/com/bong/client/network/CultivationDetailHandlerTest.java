package com.bong.client.network;

import com.bong.client.cultivation.ColorKind;
import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianBody;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillMilestoneStore;
import com.bong.client.skill.SkillSetStore;
import com.google.gson.Gson;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

public class CultivationDetailHandlerTest {

    private final CultivationDetailHandler handler = new CultivationDetailHandler();

    @BeforeEach
    void setUp() { MeridianStateStore.resetForTests(); }

    @AfterEach
    void tearDown() {
        MeridianStateStore.resetForTests();
        SkillSetStore.resetForTests();
        SkillMilestoneStore.resetForTests();
    }

    private static ServerDataEnvelope envelope(JsonObject payload) {
        payload.addProperty("type", "cultivation_detail");
        payload.addProperty("v", 1);
        String json = new Gson().toJson(payload);
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.length());
        assertTrue(r.isSuccess(), "fixture envelope should parse: " + r.errorMessage());
        return r.envelope();
    }

    private static JsonObject fullPayload(List<Boolean> opened, List<Double> rate, List<Double> cap, List<Double> integ) {
        JsonObject obj = new JsonObject();
        obj.add("opened", new Gson().toJsonTree(opened));
        obj.add("flow_rate", new Gson().toJsonTree(rate));
        obj.add("flow_capacity", new Gson().toJsonTree(cap));
        obj.add("integrity", new Gson().toJsonTree(integ));
        return obj;
    }

    private static <T> List<T> twenty(T value) {
        List<T> list = new ArrayList<>(20);
        for (int i = 0; i < 20; i++) list.add(value);
        return list;
    }

    @Test
    void appliesFullSnapshotToStore() {
        var payload = fullPayload(twenty(true), twenty(1.5), twenty(10.0), twenty(1.0));
        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertNotNull(body);
        assertEquals(20, body.allChannels().size());
        ChannelState lu = body.channel(MeridianChannel.LU);
        assertEquals(10.0, lu.capacity());
        assertEquals(1.5, lu.currentFlow());
        assertEquals(ChannelState.DamageLevel.INTACT, lu.damage());
        assertFalse(lu.blocked());
    }

    @Test
    void unopenedChannelMarkedBlocked() {
        var opened = twenty(false);
        var payload = fullPayload(opened, twenty(0.0), twenty(5.0), twenty(1.0));
        handler.handle(envelope(payload));
        assertTrue(MeridianStateStore.snapshot().channel(MeridianChannel.HT).blocked());
    }

    @Test
    void integrityMapsToDamageLevels() {
        assertEquals(ChannelState.DamageLevel.INTACT,
            CultivationDetailHandler.damageFromIntegrity(0.99));
        assertEquals(ChannelState.DamageLevel.MICRO_TEAR,
            CultivationDetailHandler.damageFromIntegrity(0.80));
        assertEquals(ChannelState.DamageLevel.TORN,
            CultivationDetailHandler.damageFromIntegrity(0.40));
        assertEquals(ChannelState.DamageLevel.SEVERED,
            CultivationDetailHandler.damageFromIntegrity(0.05));
    }

    @Test
    void integrityZeroMapsToSeveredForPlanMeridianSeveredV1() {
        // plan-meridian-severed-v1 §6 P2 inspect 染色：服务端 enforce_severed_state
        // 把永久 SEVERED 经脉的 integrity 钳到 0.0，客户端必须把 0.0 显示为 SEVERED
        // 黑色（0xFF666666）。这是 worldview §四:286「断了肺经的飞剑手就废了」的物理
        // 可见性化身——若回归到 INTACT/TORN 等级会让玩家误以为可恢复。
        assertEquals(
            ChannelState.DamageLevel.SEVERED,
            CultivationDetailHandler.damageFromIntegrity(0.0),
            "integrity == 0.0 必须映射为 SEVERED，server 永久 SEVERED 写入 0.0 后客户端染色保持一致"
        );
        assertEquals(
            0xFF666666,
            ChannelState.DamageLevel.SEVERED.color(),
            "SEVERED 颜色锁定为 0xFF666666 黑色，与 plan §6 inspect 经脉图染色规则一致"
        );
    }

    @Test
    void integrityBoundaryAtThresholdIsSeveredNotTorn() {
        // 0.10 边界：< 0.10 即 SEVERED（damageFromIntegrity 实装），刚好 0.10 进 TORN
        assertEquals(ChannelState.DamageLevel.SEVERED,
            CultivationDetailHandler.damageFromIntegrity(0.099));
        assertEquals(ChannelState.DamageLevel.TORN,
            CultivationDetailHandler.damageFromIntegrity(0.10));
    }

    @Test
    void rejectsMissingArray() {
        JsonObject bad = new JsonObject();
        bad.add("opened", new Gson().toJsonTree(twenty(true)));
        // no flow_rate / flow_capacity / integrity
        var result = handler.handle(envelope(bad));
        assertFalse(result.handled());
        assertNull(MeridianStateStore.snapshot(), "store must not be touched on validation failure");
    }

    @Test
    void rejectsWrongArrayLength() {
        var payload = fullPayload(List.of(true, true), twenty(0.0), twenty(0.0), twenty(1.0));
        var result = handler.handle(envelope(payload));
        assertFalse(result.handled());
        assertTrue(result.logMessage().contains("array length mismatch"));
    }

    @Test
    void appliesCracksCountAndContaminationTotal() {
        var cracks = new ArrayList<Integer>();
        for (int i = 0; i < 20; i++) cracks.add(i == 4 ? 3 : 0); // HT 有 3 条裂痕
        var payload = fullPayload(twenty(true), twenty(1.0), twenty(5.0), twenty(0.6));
        payload.addProperty("realm", "Condense");
        payload.add("cracks_count", new Gson().toJsonTree(cracks));
        payload.addProperty("contamination_total", 12.5);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertEquals(3, body.cracksFor(MeridianChannel.HT));
        assertEquals(0, body.cracksFor(MeridianChannel.LU));
        assertEquals(12.5, body.contaminationTotal(), 1e-9);
    }

    @Test
    void appliesLifespanPreviewWhenProvided() {
        var payload = fullPayload(twenty(true), twenty(1.0), twenty(5.0), twenty(1.0));
        JsonObject lifespan = new JsonObject();
        lifespan.addProperty("years_lived", 74.5);
        lifespan.addProperty("cap_by_realm", 80);
        lifespan.addProperty("remaining_years", 5.5);
        lifespan.addProperty("death_penalty_years", 4);
        lifespan.addProperty("tick_rate_multiplier", 2.0);
        lifespan.addProperty("is_wind_candle", true);
        payload.add("lifespan", lifespan);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertTrue(body.hasLifespanPreview());
        assertEquals(74.5, body.yearsLived(), 1e-9);
        assertEquals(80, body.lifespanCapByRealm());
        assertEquals(5.5, body.remainingYears(), 1e-9);
        assertEquals(4, body.deathPenaltyYears());
        assertEquals(2.0, body.lifespanTickRateMultiplier(), 1e-9);
        assertTrue(body.isWindCandle());
    }

    @Test
    void appliesRealmAndOpenProgressWhenProvided() {
        var opened = twenty(false);
        var openProg = new ArrayList<Double>();
        for (int i = 0; i < 20; i++) openProg.add(i / 20.0);
        var payload = fullPayload(opened, twenty(0.0), twenty(5.0), twenty(1.0));
        payload.addProperty("realm", "Induce");
        payload.add("open_progress", new Gson().toJsonTree(openProg));
        payload.add("cracks_count", new Gson().toJsonTree(twenty(0)));
        payload.addProperty("contamination_total", 0.0);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertEquals("Induce", body.realm());
        // LU=idx0 -> 0.0; HT=idx4 -> 0.2; BL=idx6 -> 0.3
        assertEquals(0.0, body.channel(MeridianChannel.LU).healProgress(), 1e-9);
        assertEquals(0.3, body.channel(MeridianChannel.BL).healProgress(), 1e-9);
        assertEquals(5, SkillSetStore.snapshot().get(SkillId.HERBALISM).cap());
        assertEquals(5, SkillSetStore.snapshot().get(SkillId.ALCHEMY).cap());
        assertEquals(5, SkillSetStore.snapshot().get(SkillId.FORGING).cap());
        assertEquals(5, SkillSetStore.snapshot().get(SkillId.COMBAT).cap());
        assertEquals(5, SkillSetStore.snapshot().get(SkillId.MINERAL).cap());
        assertEquals(5, SkillSetStore.snapshot().get(SkillId.CULTIVATION).cap());
    }

    @Test
    void appliesQiColorSnapshotWhenProvided() {
        var payload = fullPayload(twenty(true), twenty(1.0), twenty(5.0), twenty(1.0));
        payload.addProperty("qi_color_main", "Intricate");
        payload.addProperty("qi_color_secondary", "Heavy");
        payload.addProperty("qi_color_chaotic", true);
        payload.addProperty("qi_color_hunyuan", false);

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertEquals(ColorKind.Intricate, body.qiColorMain());
        assertEquals(ColorKind.Heavy, body.qiColorSecondary());
        assertTrue(body.qiColorChaotic());
        assertFalse(body.qiColorHunyuan());
    }

    @Test
    void appliesQiColorPracticeWeightsWhenProvided() {
        var payload = fullPayload(twenty(true), twenty(1.0), twenty(5.0), twenty(1.0));
        List<JsonObject> weights = new ArrayList<>();
        JsonObject heavy = new JsonObject();
        heavy.addProperty("color", "Heavy");
        heavy.addProperty("weight", 60.0);
        heavy.addProperty("ratio", 0.6);
        JsonObject solid = new JsonObject();
        solid.addProperty("color", "Solid");
        solid.addProperty("weight", 40.0);
        solid.addProperty("ratio", 0.4);
        weights.add(heavy);
        weights.add(solid);
        payload.add("practice_weights", new Gson().toJsonTree(weights));

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertEquals(60.0, body.qiColorPracticeWeights().get(ColorKind.Heavy), 1e-9);
        assertEquals(40.0, body.qiColorPracticeWeights().get(ColorKind.Solid), 1e-9);
    }

    @Test
    void openProgressIgnoredForOpenedChannels() {
        var opened = twenty(true);
        var openProg = new ArrayList<Double>();
        for (int i = 0; i < 20; i++) openProg.add(0.5);
        var payload = fullPayload(opened, twenty(1.0), twenty(5.0), twenty(1.0));
        payload.add("open_progress", new Gson().toJsonTree(openProg));
        handler.handle(envelope(payload));
        // 已打通经脉 blocked=false，healProgress 固定为 0（UI 不把 progress 渲染成治愈进度）
        assertEquals(0.0, MeridianStateStore.snapshot().channel(MeridianChannel.LU).healProgress(), 1e-9);
    }

    @Test
    void channelOrderExactly20() {
        assertEquals(20, CultivationDetailHandler.CHANNEL_ORDER.length);
        // head/tail sanity
        assertEquals(MeridianChannel.LU, CultivationDetailHandler.CHANNEL_ORDER[0]);
        assertEquals(MeridianChannel.LR, CultivationDetailHandler.CHANNEL_ORDER[11]);
        assertEquals(MeridianChannel.REN, CultivationDetailHandler.CHANNEL_ORDER[12]);
        assertEquals(MeridianChannel.YANG_WEI, CultivationDetailHandler.CHANNEL_ORDER[19]);
    }

    // plan-race-system-v1 P1c — wire 开放化：server 现附带 channel_ids 附带每个数组
    // 下标对应的 channel id 字符串，client 必须按 channel_ids keyed 查找，而非假设固定
    // CHANNEL_ORDER 位置顺序。以下测试锁死 keyed 解码行为，防止回归到位置假设。

    @Test
    void keyedDecodeIgnoresArrayPositionAndUsesChannelIds() {
        // 把 opened/flow_rate 的下标 0 塞成"pericardium"而不是 CHANNEL_ORDER[0]=lung，
        // 若 handler 仍按位置解码就会把这份数据错读成 Lung 的状态。
        var opened = new ArrayList<Boolean>(List.of(true, false));
        var rate = new ArrayList<Double>(List.of(9.0, 0.0));
        var cap = new ArrayList<Double>(List.of(20.0, 5.0));
        var integ = new ArrayList<Double>(List.of(1.0, 1.0));
        var payload = fullPayload(opened, rate, cap, integ);
        payload.add("channel_ids", new Gson().toJsonTree(List.of("pericardium", "lung")));
        handler.handle(envelope(payload));

        MeridianBody body = MeridianStateStore.snapshot();
        assertEquals(20.0, body.channel(MeridianChannel.PC).capacity(),
            "channel_ids[0]=pericardium must map array index 0 to PC, not positional CHANNEL_ORDER[0]=LU");
        assertTrue(body.channel(MeridianChannel.PC).blocked() == false);
        assertTrue(body.channel(MeridianChannel.LU).blocked(),
            "channel_ids[1]=lung with opened=false must mark LU blocked");
    }

    @Test
    void keyedDecodeSupportsNonHumanoidChannelCountAndSkipsUnknownIds() {
        // P5 飞鲸草案的合成样本：6 条经脉，其中一个 channel id 未知（客户端尚无 UI
        // 展示位）；handler 必须只跳过未知项，不 crash、不把其他项错位。
        var opened = new ArrayList<Boolean>(List.of(true, true, false, false, false, false));
        var rate = new ArrayList<Double>(List.of(1.0, 1.0, 0.0, 0.0, 0.0, 0.0));
        var cap = new ArrayList<Double>(List.of(10.0, 10.0, 10.0, 10.0, 10.0, 10.0));
        var integ = new ArrayList<Double>(List.of(1.0, 1.0, 1.0, 1.0, 1.0, 1.0));
        var payload = fullPayload(opened, rate, cap, integ);
        payload.add("channel_ids", new Gson().toJsonTree(List.of(
            "lung", "heart", "skull_channel", "tail_fin_channel", "ren", "du"
        )));
        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());

        MeridianBody body = MeridianStateStore.snapshot();
        assertFalse(body.channel(MeridianChannel.LU).blocked(), "channel_ids[0]=lung, opened[0]=true");
        assertFalse(body.channel(MeridianChannel.HT).blocked(), "channel_ids[1]=heart, opened[1]=true");
        assertTrue(body.channel(MeridianChannel.REN).blocked(), "channel_ids[4]=ren, opened[4]=false");
        assertTrue(body.channel(MeridianChannel.DU).blocked(), "channel_ids[5]=du, opened[5]=false");
        // 未知 channel id（skull_channel/tail_fin_channel）没有对应 MeridianChannel，
        // 应被静默跳过而不是撑爆 EnumMap 或抛异常——上面 handled() 断言已隐含验证。
    }

    @Test
    void resolveChannelOrderFallsBackToLegacyOrderWhenChannelIdsMissingAndLengthIs20() {
        MeridianChannel[] resolved = CultivationDetailHandler.resolveChannelOrder(null, 20);
        assertEquals(20, resolved.length);
        assertEquals(MeridianChannel.LU, resolved[0]);
        assertEquals(MeridianChannel.YANG_WEI, resolved[19]);
    }

    @Test
    void resolveChannelOrderReturnsAllNullWhenChannelIdsMissingAndLengthIsNot20() {
        // 非 humanoid 长度且缺 channel_ids 时，没有可信真源可回退——整段落 null
        // （调用方按 null 静默跳过，不假造对应关系）。
        MeridianChannel[] resolved = CultivationDetailHandler.resolveChannelOrder(null, 6);
        assertEquals(6, resolved.length);
        for (MeridianChannel ch : resolved) {
            assertNull(ch);
        }
    }

    @Test
    void skillCapForRealmMatchesPlanSectionFour() {
        assertEquals(3, CultivationDetailHandler.skillCapForRealm("Awaken"));
        assertEquals(5, CultivationDetailHandler.skillCapForRealm("Induce"));
        assertEquals(7, CultivationDetailHandler.skillCapForRealm("Condense"));
        assertEquals(8, CultivationDetailHandler.skillCapForRealm("Solidify"));
        assertEquals(9, CultivationDetailHandler.skillCapForRealm("Spirit"));
        assertEquals(10, CultivationDetailHandler.skillCapForRealm("Void"));
        assertNull(CultivationDetailHandler.skillCapForRealm("MysteryRealm"));
    }

    @Test
    void targetMeridianParsedCorrectly() {
        var opened = twenty(false);
        var openProg = twenty(0.0);
        openProg.set(4, 0.65); // Heart = index 4
        var payload = fullPayload(opened, twenty(0.0), twenty(5.0), twenty(1.0));
        payload.add("open_progress", new Gson().toJsonTree(openProg));
        payload.addProperty("target_meridian", "heart");
        handler.handle(envelope(payload));
        MeridianBody body = MeridianStateStore.snapshot();
        assertEquals(MeridianChannel.HT, body.targetMeridian());
        assertEquals(0.65, body.channel(MeridianChannel.HT).healProgress(), 1e-9);
    }

    @Test
    void targetMeridianNullWhenAbsent() {
        var payload = fullPayload(twenty(true), twenty(1.0), twenty(5.0), twenty(1.0));
        handler.handle(envelope(payload));
        assertNull(MeridianStateStore.snapshot().targetMeridian());
    }

    @Test
    void targetMeridianNullWhenOutOfRange() {
        var payload = fullPayload(twenty(false), twenty(0.0), twenty(5.0), twenty(1.0));
        payload.addProperty("target_meridian", "unknown_channel_id");
        handler.handle(envelope(payload));
        assertNull(MeridianStateStore.snapshot().targetMeridian());
    }

    @Test
    void targetMeridianExtraordinary() {
        var payload = fullPayload(twenty(false), twenty(0.0), twenty(5.0), twenty(1.0));
        payload.addProperty("target_meridian", "ren");
        handler.handle(envelope(payload));
        assertEquals(MeridianChannel.REN, MeridianStateStore.snapshot().targetMeridian());
    }

    @Test
    void appliesSkillMilestonesWhenPresent() {
        var payload = fullPayload(twenty(true), twenty(1.0), twenty(5.0), twenty(1.0));
        payload.addProperty("realm", "Spirit");
        payload.add("open_progress", new Gson().toJsonTree(twenty(1.0)));
        payload.add("cracks_count", new Gson().toJsonTree(twenty(0)));
        payload.addProperty("contamination_total", 0.0);
        payload.addProperty("recent_skill_milestones_summary", "t82000:skill:alchemy:lv3");

        JsonObject milestone = new JsonObject();
        milestone.addProperty("skill", "alchemy");
        milestone.addProperty("new_lv", 3);
        milestone.addProperty("achieved_at", 82000);
        milestone.addProperty("narration", "炉火渐驯，丹性稍明。 (alchemy Lv 2 → 3)");
        milestone.addProperty("total_xp_at", 1400);
        payload.add("skill_milestones", new Gson().toJsonTree(java.util.List.of(milestone)));

        var result = handler.handle(envelope(payload));
        assertTrue(result.handled(), result.logMessage());
        assertEquals(1, SkillMilestoneStore.snapshot().size());
        assertEquals(SkillId.ALCHEMY, SkillMilestoneStore.snapshot().get(0).skill());
        assertEquals(3, SkillMilestoneStore.snapshot().get(0).newLv());
        assertEquals("t82000:skill:alchemy:lv3", SkillMilestoneStore.summary());
    }
}
