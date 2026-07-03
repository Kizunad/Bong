package com.bong.client.network;

import com.bong.client.combat.CastOutcome;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
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
        UnifiedEventStore.resetForTests();
    }
    @AfterEach
    void tearDown() {
        CastStateStore.resetForTests();
        UnifiedEventStore.resetForTests();
    }

    @Test
    void appliesCastingPhase() {
        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"casting","slot":3,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"none"}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        CastState state = CastStateStore.snapshot();
        assertEquals(CastState.Phase.CASTING, state.phase());
        assertEquals(3, state.slot());
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
        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"casting","slot":42,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"none"}
            """));

        assertFalse(dispatch.handled());
    }

    @Test
    void completedSkillBarCastDoesNotRelabelNextQuickSlotCast() {
        CastStateStore.beginSkillBarCast(2, 500, 1000L);
        CastStateStore.complete(1500L);

        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"casting","slot":2,
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
    void everyRejectOutcomePublishesFriendlyWarningText() {
        // 通用性：所有拒绝原因都弹一条 SYSTEM 频道警示，文案非空 = CastOutcome.warningText()。
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
            UnifiedEventStore.resetForTests();
            new CastSyncHandler().handle(parseEnvelope("""
                {"v":1,"type":"cast_sync","phase":"idle","slot":0,
                 "duration_ms":0,"started_at_ms":1700000000000,"outcome":"%s"}
                """.formatted(c.wire())));
            List<UnifiedEvent> events = UnifiedEventStore.stream().snapshot();
            assertEquals(1, events.size(),
                "拒绝 '" + c.wire() + "' 应弹恰好一条警示；实际 " + events.size() + " 条");
            UnifiedEvent e = events.get(0);
            assertEquals(UnifiedEvent.Channel.SYSTEM, e.channel(),
                "技能警示走 SYSTEM 频道（瞬态，非战斗刷屏）");
            assertEquals(c.expectedText(), e.text(),
                "拒绝 '" + c.wire() + "' 文案应为「" + c.expectedText() + "」，实际「" + e.text() + "」");
        }
    }

    @Test
    void nonRejectionOutcomesDoNotPublishWarning() {
        // happy path / 中断 outcome 不应弹警示——警示只为"前置不满足没放出来"。
        for (String wire : List.of("none", "completed", "interrupt_movement",
                                   "interrupt_contam", "interrupt_control",
                                   "user_cancel", "death")) {
            UnifiedEventStore.resetForTests();
            new CastSyncHandler().handle(parseEnvelope("""
                {"v":1,"type":"cast_sync","phase":"complete","slot":0,
                 "duration_ms":500,"started_at_ms":1700000000000,"outcome":"%s"}
                """.formatted(wire)));
            assertEquals(0, UnifiedEventStore.stream().snapshot().size(),
                "outcome '" + wire + "' 非拒绝，不应弹技能警示");
        }
    }

    @Test
    void repeatedSameRejectionFoldsInsteadOfSpamming() {
        // 连点同一被拒技能：1.5s 折叠窗口内同 source_tag+text 折叠为一条，避免刷屏。
        for (int i = 0; i < 5; i++) {
            new CastSyncHandler().handle(parseEnvelope("""
                {"v":1,"type":"cast_sync","phase":"idle","slot":0,
                 "duration_ms":0,"started_at_ms":1700000000000,"outcome":"reject_no_weapon"}
                """));
        }
        List<UnifiedEvent> events = UnifiedEventStore.stream().snapshot();
        assertEquals(1, events.size(),
            "连按 5 次缺武器应折叠成 1 条（按住不放也不刷屏）；实际 " + events.size() + " 条");
        assertEquals("缺少武器", events.get(0).text());
        assertTrue(events.get(0).foldCount() >= 2,
            "折叠计数应记录重复次数（×N）；实际 foldCount=" + events.get(0).foldCount());
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

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
