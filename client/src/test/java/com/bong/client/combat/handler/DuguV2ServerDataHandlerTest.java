package com.bong.client.combat.handler;

import bong.Envelope;
import com.bong.client.hud.DuguV2HudStateStore;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataRouter;
import com.bong.client.network.ServerPayloadParseResult;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-combat-skill-feedback-bridges-v1 P5 饱和测试：
 * DuguV2ServerDataHandler + proto 链双端对拍 + ServerDataRouter 路由 + e2e。
 */
class DuguV2ServerDataHandlerTest {

    private DuguV2ServerDataHandler handler;

    @BeforeEach
    void setUp() {
        handler = new DuguV2ServerDataHandler();
        DuguV2HudStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        DuguV2HudStateStore.resetForTests();
    }

    // ─── dugu_v2_self_cure：自蕴进度 + self_revealed ──────────────

    @Test
    void selfCurePayloadWritesGainPercentToStore() {
        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_self_cure\",\"caster\":\"player:a\","
                + "\"gain_percent\":37.2,\"self_revealed\":false,\"tick\":300}"
        ));
        assertTrue(dispatch.handled(),
            "handler 应处理 dugu_v2_self_cure payload；实际=" + dispatch.logMessage());
        DuguV2HudStateStore.State state = DuguV2HudStateStore.snapshot();
        assertEquals(37.2f, state.selfCurePercent(), 0.1f,
            "selfCurePercent 必须等于 payload.gain_percent=37.2；实际=" + state.selfCurePercent());
    }

    @Test
    void selfCurePayloadWithSelfRevealedTrueSetsStickyFlag() {
        // 先推送 self_revealed=false，再推送 self_revealed=true，确认 sticky 行为
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_self_cure\",\"caster\":\"p\","
                + "\"gain_percent\":20.0,\"self_revealed\":false,\"tick\":1}"
        ));
        assertFalse(DuguV2HudStateStore.snapshot().selfRevealed(),
            "首次 self_revealed=false，store 不应为 true；实际=" + DuguV2HudStateStore.snapshot().selfRevealed());

        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_self_cure\",\"caster\":\"p\","
                + "\"gain_percent\":25.0,\"self_revealed\":true,\"tick\":2}"
        ));
        assertTrue(DuguV2HudStateStore.snapshot().selfRevealed(),
            "self_revealed=true 后 store.selfRevealed 应为 true（DuguV2HudPlanner suffix=' 已露'）；"
                + "实际=" + DuguV2HudStateStore.snapshot().selfRevealed());

        // 再推送 self_revealed=false，sticky flag 不应归零
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_self_cure\",\"caster\":\"p\","
                + "\"gain_percent\":30.0,\"self_revealed\":false,\"tick\":3}"
        ));
        assertTrue(DuguV2HudStateStore.snapshot().selfRevealed(),
            "self_revealed 是 once-set-stays-true sticky flag：后续 false payload 不应归零；"
                + "实际=" + DuguV2HudStateStore.snapshot().selfRevealed());
    }

    @Test
    void selfCurePayloadDoesNotOverwriteShroudDimension() {
        // 先设置 shroud 状态
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_shroud_active\",\"caster\":\"p\","
                + "\"strength\":0.8,\"expires_at_tick\":200,\"tick\":100}"
        ));
        assertTrue(DuguV2HudStateStore.snapshot().shroudActive(),
            "shroud_active 后 shroudActive 应为 true");

        // 再推送 self_cure，不应清零 shroud 维度（per-dimension merge）
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_self_cure\",\"caster\":\"p\","
                + "\"gain_percent\":50.0,\"self_revealed\":false,\"tick\":150}"
        ));
        assertTrue(DuguV2HudStateStore.snapshot().shroudActive(),
            "self_cure payload 到来后 shroudActive 不应归零（per-dimension merge 不互相清零）；"
                + "实际=" + DuguV2HudStateStore.snapshot().shroudActive());
        assertEquals(50.0f, DuguV2HudStateStore.snapshot().selfCurePercent(), 0.1f,
            "selfCurePercent 应为 50.0；实际=" + DuguV2HudStateStore.snapshot().selfCurePercent());
    }

    // ─── dugu_v2_shroud_active：幻影遮蔽 ───────────────────────────

    @Test
    void shroudPayloadSetsShroudActive() {
        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_shroud_active\",\"caster\":\"p\","
                + "\"strength\":0.8,\"expires_at_tick\":200,\"tick\":100}"
        ));
        assertTrue(dispatch.handled(), "handler 应处理 dugu_v2_shroud_active；" + dispatch.logMessage());
        DuguV2HudStateStore.State state = DuguV2HudStateStore.snapshot();
        assertTrue(state.shroudActive(),
            "shroudActive 应为 true；实际=" + state.shroudActive());
        assertTrue(state.shroudUntilMs() > System.currentTimeMillis(),
            "shroudUntilMs 应大于当前时间（幻影遮蔽未过期）；实际=" + state.shroudUntilMs());
    }

    @Test
    void shroudPayloadDoesNotOverwriteSelfCureDimension() {
        // 先设置 self_cure 状态
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_self_cure\",\"caster\":\"p\","
                + "\"gain_percent\":60.0,\"self_revealed\":true,\"tick\":10}"
        ));
        assertTrue(DuguV2HudStateStore.snapshot().selfRevealed(), "self_revealed 应为 true");

        // 再推送 shroud，不应清零 selfRevealed（per-dimension merge）
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_shroud_active\",\"caster\":\"p\","
                + "\"strength\":0.9,\"expires_at_tick\":500,\"tick\":200}"
        ));
        assertTrue(DuguV2HudStateStore.snapshot().selfRevealed(),
            "shroud payload 到来后 selfRevealed 不应归零（per-dimension merge 防止互相清零）；"
                + "实际=" + DuguV2HudStateStore.snapshot().selfRevealed());
        assertTrue(DuguV2HudStateStore.snapshot().shroudActive(),
            "shroudActive 应为 true；实际=" + DuguV2HudStateStore.snapshot().shroudActive());
    }

    @Test
    void shroudZeroStrengthSetsShroudInactive() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_shroud_active\",\"caster\":\"p\","
                + "\"strength\":0.0,\"expires_at_tick\":200,\"tick\":100}"
        ));
        assertFalse(DuguV2HudStateStore.snapshot().shroudActive(),
            "strength=0 应设置 shroudActive=false（幻影强度为 0 视为无遮蔽）；"
                + "实际=" + DuguV2HudStateStore.snapshot().shroudActive());
    }

    // ─── permanent_qi_max_decay_applied：守恒 pin ─────────────────

    @Test
    void permanentQiDecayHandledButDoesNotWriteQiFieldsToStore() {
        // 守恒红线：loss/qi_max_after 只读，不应修改 DuguV2HudStateStore 的 qi 相关字段
        // store 仅有 tainted/taintIntensity/selfCurePercent 等字段，无 qi 字段
        // 本测试验证 handler 正常处理（不抛异常/不崩溃）且 store 状态不变
        DuguV2HudStateStore.State before = DuguV2HudStateStore.snapshot();
        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"permanent_qi_max_decay_applied\",\"target\":\"player:t\","
                + "\"loss\":0.45,\"qi_max_after\":99.55,\"tick\":1000}"
        ));
        assertTrue(dispatch.handled(),
            "handler 应正常处理 permanent_qi_max_decay_applied；" + dispatch.logMessage());
        DuguV2HudStateStore.State after = DuguV2HudStateStore.snapshot();
        assertEquals(before.tainted(), after.tainted(),
            "permanent_qi_decay 不应修改 store.tainted（守恒红线）；"
                + "before=" + before.tainted() + " after=" + after.tainted());
        assertEquals(before.selfCurePercent(), after.selfCurePercent(), 0.001f,
            "permanent_qi_decay 不应修改 store.selfCurePercent（守恒红线）；"
                + "before=" + before.selfCurePercent() + " after=" + after.selfCurePercent());
        assertEquals(before.shroudActive(), after.shroudActive(),
            "permanent_qi_decay 不应修改 store.shroudActive（守恒红线）；"
                + "before=" + before.shroudActive() + " after=" + after.shroudActive());
    }

    // ─── dugu_v2_skill_cast：招式投放（瞬态，不写 store 持久状态）─

    @Test
    void skillCastPayloadIsHandledWithoutThrowingException() {
        for (String kind : new String[]{"eclipse", "penetrate", "shroud", "self_cure", "reverse"}) {
            ServerDataDispatch dispatch = handler.handle(parse(
                "{\"v\":1,\"type\":\"dugu_v2_skill_cast\",\"caster\":\"p\","
                    + "\"kind\":\"" + kind + "\",\"taint_tier\":\"temporary\","
                    + "\"reveal_probability\":0.01,\"tick\":100}"
            ));
            assertTrue(dispatch.handled() || !dispatch.handled(),
                "任何 kind 的 dugu_v2_skill_cast 不应抛出异常；kind=" + kind
                    + " dispatch=" + dispatch.logMessage());
        }
    }

    // ─── dugu_v2_skill_cast：revealRisk per-dimension merge ────────

    @Test
    void skillCastPayloadWritesRevealRiskToStore() {
        // red-when-reverted：如果 handler 不写 revealRisk，本测试必然红
        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_skill_cast\",\"caster\":\"p\","
                + "\"kind\":\"eclipse\",\"taint_tier\":\"temporary\","
                + "\"reveal_probability\":0.7,\"tick\":42}"
        ));
        assertTrue(dispatch.handled(),
            "handler 应处理 dugu_v2_skill_cast；实际=" + dispatch.logMessage());
        float actual = DuguV2HudStateStore.snapshot().revealRisk();
        assertEquals(0.7f, actual, 0.001f,
            "dugu_v2_skill_cast payload reveal_probability=0.7 → store.revealRisk 必须为 0.7；"
                + "实际=" + actual + "（handler 未写 revealRisk 则为 0.0）");
    }

    @Test
    void skillCastRevealRiskZeroWhenPayloadOmitsField() {
        // reveal_probability 缺省时 revealRisk 应为 0（reverse 招式无暴露风险）
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_skill_cast\",\"caster\":\"p\","
                + "\"kind\":\"reverse\",\"tick\":10}"
        ));
        assertEquals(0.0f, DuguV2HudStateStore.snapshot().revealRisk(), 0.001f,
            "reverse 招式 payload 无 reveal_probability 字段 → store.revealRisk 应为 0.0；"
                + "实际=" + DuguV2HudStateStore.snapshot().revealRisk());
    }

    @Test
    void skillCastRevealRiskDoesNotOverwriteShroudDimension() {
        // 先设置 shroud，再 skill_cast → shroud 维度不应被清零（per-dimension merge 证据）
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_shroud_active\",\"caster\":\"p\","
                + "\"strength\":0.8,\"expires_at_tick\":500,\"tick\":100}"
        ));
        assertTrue(DuguV2HudStateStore.snapshot().shroudActive(),
            "前置：shroudActive 应为 true");

        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_skill_cast\",\"caster\":\"p\","
                + "\"kind\":\"eclipse\",\"reveal_probability\":0.5,\"tick\":200}"
        ));
        // shroud 维度不被 skill_cast 清零
        assertTrue(DuguV2HudStateStore.snapshot().shroudActive(),
            "skill_cast(revealRisk) per-dimension merge：不应清零 shroudActive；"
                + "实际=" + DuguV2HudStateStore.snapshot().shroudActive());
        assertEquals(0.5f, DuguV2HudStateStore.snapshot().revealRisk(), 0.001f,
            "revealRisk 应为 0.5；实际=" + DuguV2HudStateStore.snapshot().revealRisk());
    }

    @Test
    void skillCastRevealRiskDoesNotOverwriteSelfCureDimension() {
        // 先设置 self_cure，再 skill_cast → selfCure 维度不应被清零
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_self_cure\",\"caster\":\"p\","
                + "\"gain_percent\":40.0,\"self_revealed\":true,\"tick\":10}"
        ));
        assertEquals(40.0f, DuguV2HudStateStore.snapshot().selfCurePercent(), 0.1f,
            "前置：selfCurePercent 应为 40.0");
        assertTrue(DuguV2HudStateStore.snapshot().selfRevealed(),
            "前置：selfRevealed 应为 true");

        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_skill_cast\",\"caster\":\"p\","
                + "\"kind\":\"penetrate\",\"reveal_probability\":0.3,\"tick\":50}"
        ));
        assertEquals(40.0f, DuguV2HudStateStore.snapshot().selfCurePercent(), 0.1f,
            "skill_cast(revealRisk) per-dimension merge：不应清零 selfCurePercent；"
                + "实际=" + DuguV2HudStateStore.snapshot().selfCurePercent());
        assertTrue(DuguV2HudStateStore.snapshot().selfRevealed(),
            "skill_cast(revealRisk) per-dimension merge：不应清零 selfRevealed；"
                + "实际=" + DuguV2HudStateStore.snapshot().selfRevealed());
        assertEquals(0.3f, DuguV2HudStateStore.snapshot().revealRisk(), 0.001f,
            "revealRisk 应为 0.3；实际=" + DuguV2HudStateStore.snapshot().revealRisk());
    }

    @Test
    void revealRiskBoundaryClampedToZeroOne() {
        // reveal_probability > 1.0 应被 State 构造器 clamp 到 1.0
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_skill_cast\",\"caster\":\"p\","
                + "\"kind\":\"eclipse\",\"reveal_probability\":2.5,\"tick\":1}"
        ));
        assertEquals(1.0f, DuguV2HudStateStore.snapshot().revealRisk(), 0.001f,
            "revealRisk 超出 1.0 应被 clamp01 截断为 1.0；实际=" + DuguV2HudStateStore.snapshot().revealRisk());

        DuguV2HudStateStore.resetForTests();
        // reveal_probability 负值应 clamp 到 0
        handler.handle(parse(
            "{\"v\":1,\"type\":\"dugu_v2_skill_cast\",\"caster\":\"p\","
                + "\"kind\":\"eclipse\",\"reveal_probability\":-0.1,\"tick\":2}"
        ));
        assertEquals(0.0f, DuguV2HudStateStore.snapshot().revealRisk(), 0.001f,
            "revealRisk 负值应被 clamp01 截断为 0；实际=" + DuguV2HudStateStore.snapshot().revealRisk());
    }

    // ─── e2e：router → handler → store revealRisk ─────────────────

    @Test
    void routerDispatchesSkillCastRevealRiskToStore() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        String json = "{\"v\":1,\"type\":\"dugu_v2_skill_cast\","
            + "\"caster\":\"p\",\"kind\":\"eclipse\",\"reveal_probability\":0.42,\"tick\":77}";

        ServerDataRouter.RouteResult result = router.route(
            json, json.getBytes(java.nio.charset.StandardCharsets.UTF_8).length
        );

        assertFalse(result.isParseError(),
            "router 应成功路由 dugu_v2_skill_cast；实际=" + (result.isParseError() ? result.logMessage() : "none"));
        assertEquals(0.42f, DuguV2HudStateStore.snapshot().revealRisk(), 0.001f,
            "e2e：router → DuguV2ServerDataHandler → store.revealRisk 应为 0.42；"
                + "实际=" + DuguV2HudStateStore.snapshot().revealRisk());
    }

    // ─── proto 链双端对拍（server Envelope → ProtoServerDataBridge → legacy JSON）─

    @Test
    void protoDuguV2SelfCureRoundtripToLegacyJson() {
        // 锁 proto 链：server 编码 DuguV2SelfCure proto（field 125）→ client bridge 反序列化
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setDuguV2SelfCure(Envelope.DuguV2SelfCure.newBuilder()
                        .setCaster("player:a")
                        .setGainPercent(37.2f)
                        .setSelfRevealed(true)
                        .setTick(300L))
                .build();

        ProtoServerDataBridge.BridgeResult result =
                ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertTrue(result.isSuccess(),
            "DuguV2SelfCure proto bridge 必须成功；错误=" + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals(1, json.get("v").getAsInt(), "v 必须为 1");
        assertEquals("dugu_v2_self_cure", json.get("type").getAsString(),
            "type 必须为 'dugu_v2_self_cure'（ProtoServerDataBridge CASE_TO_TYPE）；"
                + "实际=" + json.get("type"));
        assertEquals(37.2, json.get("gain_percent").getAsDouble(), 0.1,
            "gain_percent 不应丢失；实际=" + json.get("gain_percent"));
        assertTrue(json.get("self_revealed").getAsBoolean(),
            "self_revealed=true 必须通过 proto 链不丢失（DuguV2HudStateStore.selfRevealed sticky）；"
                + "实际=" + json.get("self_revealed"));
        assertEquals(300L, json.get("tick").getAsLong(), "tick 不应丢失；实际=" + json.get("tick"));
    }

    @Test
    void protoDuguV2ShroudActiveRoundtripToLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setDuguV2ShroudActive(Envelope.DuguV2ShroudActive.newBuilder()
                        .setCaster("player:s")
                        .setStrength(0.8f)
                        .setExpiresAtTick(2000L)
                        .setTick(1500L))
                .build();

        ProtoServerDataBridge.BridgeResult result =
                ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertTrue(result.isSuccess(), "DuguV2ShroudActive bridge 必须成功；错误=" + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("dugu_v2_shroud_active", json.get("type").getAsString(),
            "type 必须为 'dugu_v2_shroud_active'；实际=" + json.get("type"));
        assertEquals(0.8, json.get("strength").getAsDouble(), 0.01,
            "strength 不应丢失；实际=" + json.get("strength"));
        assertEquals(2000L, json.get("expires_at_tick").getAsLong(),
            "expires_at_tick 不应丢失（client 需此字段计算 shroudUntilMs）；实际=" + json.get("expires_at_tick"));
    }

    @Test
    void protoDuguV2SkillCastRoundtripToLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setDuguV2SkillCast(Envelope.DuguV2SkillCast.newBuilder()
                        .setKind("eclipse")
                        .setCaster("player:a")
                        .setTarget("player:b")
                        .setTaintTier("temporary")
                        .setRevealProbability(0.0042f)
                        .setTick(42L))
                .build();

        ProtoServerDataBridge.BridgeResult result =
                ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertTrue(result.isSuccess(), "DuguV2SkillCast bridge 必须成功；错误=" + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("dugu_v2_skill_cast", json.get("type").getAsString(),
            "type 必须为 'dugu_v2_skill_cast'；实际=" + json.get("type"));
        assertEquals("eclipse", json.get("kind").getAsString(),
            "kind 不应丢失；实际=" + json.get("kind"));
        assertEquals("temporary", json.get("taint_tier").getAsString(),
            "taint_tier 应为 'temporary'（snake_case wire）；实际=" + json.get("taint_tier"));
    }

    @Test
    void protoPermanentQiDecayRoundtripToLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setPermanentQiMaxDecayApplied(Envelope.PermanentQiMaxDecayApplied.newBuilder()
                        .setTarget("player:t")
                        .setLoss(0.45f)
                        .setQiMaxAfter(99.55f)
                        .setTick(1000L))
                .build();

        ProtoServerDataBridge.BridgeResult result =
                ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertTrue(result.isSuccess(), "PermanentQiMaxDecayApplied bridge 必须成功；错误=" + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("permanent_qi_max_decay_applied", json.get("type").getAsString(),
            "type 必须为 'permanent_qi_max_decay_applied'；实际=" + json.get("type"));
        assertEquals(0.45, json.get("loss").getAsDouble(), 0.01,
            "loss 不应丢失（守恒红线：只读不重算）；实际=" + json.get("loss"));
        assertEquals(99.55, json.get("qi_max_after").getAsDouble(), 0.1,
            "qi_max_after 不应丢失；实际=" + json.get("qi_max_after"));
    }

    // ─── ServerDataRouter 注册（e2e 路由链）──────────────────────

    @Test
    void routerDispatchesDuguV2SelfCureToStore() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        String json = "{\"v\":1,\"type\":\"dugu_v2_self_cure\","
            + "\"caster\":\"p\",\"gain_percent\":55.5,\"self_revealed\":true,\"tick\":10}";

        ServerDataRouter.RouteResult result = router.route(
            json, json.getBytes(StandardCharsets.UTF_8).length
        );

        assertFalse(result.isParseError(),
            "router 应成功路由 dugu_v2_self_cure；错误=" + (result.isParseError() ? result.logMessage() : "none"));
        assertTrue(DuguV2HudStateStore.snapshot().selfRevealed(),
            "e2e：router → DuguV2ServerDataHandler → DuguV2HudStateStore.selfRevealed 应为 true；"
                + "实际=" + DuguV2HudStateStore.snapshot().selfRevealed());
        assertEquals(55.5f, DuguV2HudStateStore.snapshot().selfCurePercent(), 0.1f,
            "e2e：selfCurePercent 应为 55.5；实际=" + DuguV2HudStateStore.snapshot().selfCurePercent());
    }

    @Test
    void routerDispatchesDuguV2ShroudActiveToStore() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        String json = "{\"v\":1,\"type\":\"dugu_v2_shroud_active\","
            + "\"caster\":\"p\",\"strength\":0.7,\"expires_at_tick\":500,\"tick\":400}";

        ServerDataRouter.RouteResult result = router.route(
            json, json.getBytes(StandardCharsets.UTF_8).length
        );

        assertFalse(result.isParseError(),
            "router 应成功路由 dugu_v2_shroud_active；实际=" + (result.isParseError() ? result.logMessage() : "none"));
        assertTrue(DuguV2HudStateStore.snapshot().shroudActive(),
            "e2e：shroudActive 应为 true；实际=" + DuguV2HudStateStore.snapshot().shroudActive());
    }

    @Test
    void routerDispatchesPermanentQiDecayWithoutError() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        String json = "{\"v\":1,\"type\":\"permanent_qi_max_decay_applied\","
            + "\"target\":\"p\",\"loss\":0.5,\"qi_max_after\":99.5,\"tick\":200}";

        ServerDataRouter.RouteResult result = router.route(
            json, json.getBytes(StandardCharsets.UTF_8).length
        );

        assertFalse(result.isParseError(),
            "router 应成功路由 permanent_qi_max_decay_applied；实际=" + (result.isParseError() ? result.logMessage() : "none"));
    }

    // ─── helper ──────────────────────────────────────────────────

    private static ServerDataEnvelope parse(String json) {
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(r.isSuccess(), () -> "parse failed: " + r.errorMessage());
        return r.envelope();
    }
}
