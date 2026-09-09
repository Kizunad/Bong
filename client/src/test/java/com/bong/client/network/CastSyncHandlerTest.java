package com.bong.client.network;

import com.bong.client.combat.CastOutcome;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.hud.BongHudOrchestrator;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongToast;
import com.bong.client.hud.HudRenderCommand;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class CastSyncHandlerTest {
    @BeforeEach
    void setUp() {
        CastStateStore.resetForTests();
        BongToast.resetForTests();
    }
    @AfterEach
    void tearDown() {
        CastStateStore.resetForTests();
        BongToast.resetForTests();
    }

    @Test
    void appliesCastingPhase() {
        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"casting","slot":1,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"none"}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        CastState state = CastStateStore.snapshot();
        assertEquals(CastState.Phase.CASTING, state.phase());
        assertEquals(1, state.slot());
        assertEquals(1500, state.durationMs());
    }

    @Test
    void appliesInterruptPhaseWithOutcome() {
        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"interrupt","slot":0,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"interrupt_contam"}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        CastState state = CastStateStore.snapshot();
        assertEquals(CastState.Phase.INTERRUPT, state.phase());
        assertEquals(CastOutcome.INTERRUPT_CONTAM, state.outcome());
    }

    @Test
    void rejectsUnknownPhase() {
        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"sleeping","slot":0,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"none"}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("unknown phase"));
    }

    @Test
    void rejectsOutOfRangeSlot() {
        CastStateStore.beginSkillBarCast(0, 1500, 1000L);
        CastState active = CastStateStore.snapshot();
        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"casting","slot":2,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"none"}
            """));

        assertFalse(dispatch.handled());
        assertEquals(active, CastStateStore.snapshot(), "越界回执不得覆盖当前施法");
    }

    @Test
    void completedSkillBarCastDoesNotRelabelNextQuickSlotCast() {
        CastStateStore.beginSkillBarCast(1, 500, 1000L);
        CastStateStore.complete(1500L);

        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"casting","slot":1,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"none"}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(CastState.Source.QUICK_SLOT, CastStateStore.snapshot().source());
    }

    // ─── 通用技能警示 HUD（plan-skill-warn-hud）─────────────────────────────────

    @Test
    void rejectOutcomesParseToTheirEnumVariants() {
        // 每个 reject wire 串必须 parse 成对应 CastOutcome（否则警示文案错位）。
        record Case(String wire, CastOutcome expected) {}
        List<Case> cases = List.of(
            new Case("meridian_gated", CastOutcome.MERIDIAN_GATED),
            new Case("reject_qi_insufficient", CastOutcome.REJECT_QI_INSUFFICIENT),
            new Case("reject_on_cooldown", CastOutcome.REJECT_ON_COOLDOWN),
            new Case("reject_invalid_target", CastOutcome.REJECT_INVALID_TARGET),
            new Case("reject_in_recovery", CastOutcome.REJECT_IN_RECOVERY),
            new Case("reject_realm_too_low", CastOutcome.REJECT_REALM_TOO_LOW),
            new Case("reject_no_weapon", CastOutcome.REJECT_NO_WEAPON),
            new Case("reject_technique_inactive", CastOutcome.REJECT_TECHNIQUE_INACTIVE)
        );
        for (Case c : cases) {
            CastStateStore.resetForTests();
            ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
                {"v":1,"type":"cast_sync","phase":"idle","slot":0,
                 "duration_ms":0,"started_at_ms":1700000000000,"outcome":"%s"}
                """.formatted(c.wire())));
            assertTrue(dispatch.handled(), dispatch.logMessage());
            assertEquals(c.expected(), CastStateStore.snapshot().outcome(),
                "wire '" + c.wire() + "' 应 parse 成 " + c.expected()
                + "（实际 " + CastStateStore.snapshot().outcome() + "）—— 警示文案映射依赖此");
        }
    }

    @Test
    void everyRejectOutcomeRendersFriendlyWarningText() {
        // 经过真实 HUD 组装验证可见文案，避免只写入无人绘制的事件缓冲也通过。
        record Case(String wire, String expectedText) {}
        List<Case> cases = List.of(
            new Case("meridian_gated", "经脉受损"),
            new Case("reject_qi_insufficient", "真元不足"),
            new Case("reject_on_cooldown", "冷却中"),
            new Case("reject_invalid_target", "目标无效"),
            new Case("reject_in_recovery", "尚未恢复"),
            new Case("reject_realm_too_low", "境界不足"),
            new Case("reject_no_weapon", "缺少武器"),
            new Case("reject_technique_inactive", "招式未激活")
        );
        for (Case c : cases) {
            BongToast.resetForTests();
            new CastSyncHandler().handle(parseEnvelope("""
                {"v":1,"type":"cast_sync","phase":"idle","slot":0,
                 "duration_ms":0,"started_at_ms":1700000000000,"outcome":"%s"}
                """.formatted(c.wire())));
            List<HudRenderCommand> warnings = renderedWarnings(System.currentTimeMillis());
            assertEquals(1, warnings.size(), "拒绝 '" + c.wire() + "' 必须显示一条警示");
            assertEquals(c.expectedText(), warnings.get(0).text(),
                "拒绝 '" + c.wire() + "' 必须显示具体原因");
        }
    }

    @Test
    void nonRejectionOutcomesDoNotPublishWarning() {
        // happy path / 中断 outcome 不应弹警示——警示只为"前置不满足没放出来"。
        for (String wire : List.of("none", "completed", "interrupt_movement",
                                   "interrupt_contam", "interrupt_control",
                                   "user_cancel", "death")) {
            BongToast.resetForTests();
            new CastSyncHandler().handle(parseEnvelope("""
                {"v":1,"type":"cast_sync","phase":"complete","slot":0,
                 "duration_ms":500,"started_at_ms":1700000000000,"outcome":"%s"}
                """.formatted(wire)));
            assertTrue(renderedWarnings(System.currentTimeMillis()).isEmpty(),
                "outcome '" + wire + "' 非拒绝，不应弹技能警示");
        }
    }

    @Test
    void repeatedSameRejectionRendersSingleWarningUntilExpiry() {
        for (int i = 0; i < 5; i++) {
            new CastSyncHandler().handle(parseEnvelope("""
                {"v":1,"type":"cast_sync","phase":"idle","slot":0,
                 "duration_ms":0,"started_at_ms":1700000000000,"outcome":"reject_no_weapon"}
                """));
        }
        long now = System.currentTimeMillis();
        long expiresAt = BongToast.current(now).expiresAtMillis();
        List<HudRenderCommand> warnings = renderedWarnings(now);
        assertEquals(1, warnings.size(), "连续拒绝只能显示一条提示，不能堆叠刷屏");
        assertEquals("缺少武器", warnings.get(0).text());
        assertTrue(renderedWarnings(expiresAt).isEmpty(), "拒绝提示到期后必须从 HUD 消失");
    }

    @Test
    void everyRejectionOutcomeHasNonNullWarningText() {
        // 不变量：isCastRejection() 为 true 的 outcome 必有非空 warningText，
        // 否则会出现"判定为拒绝却没文案"的静默漏洞。
        for (CastOutcome o : CastOutcome.values()) {
            if (o.isCastRejection()) {
                String text = o.warningText();
                assertNotNull(text, o + " 是拒绝 outcome 但 warningText() 返回 null");
                assertFalse(text.isEmpty(), o + " 的 warningText 不应为空串");
            } else {
                assertEquals(null, o.warningText(),
                    o + " 非拒绝 outcome，warningText 应为 null（调用方据此跳过弹提示）");
            }
        }
    }

    private static List<HudRenderCommand> renderedWarnings(long nowMillis) {
        return BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(), nowMillis, text -> text.length() * 6, 220
        ).stream().filter(HudRenderCommand::isToast).toList();
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
