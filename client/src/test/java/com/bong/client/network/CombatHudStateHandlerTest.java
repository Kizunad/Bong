package com.bong.client.network;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class CombatHudStateHandlerTest {
    @BeforeEach
    void setUp() {
        CombatHudStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        CombatHudStateStore.resetForTests();
    }

    @Test
    void appliesValidPayloadToStore() {
        ServerDataDispatch dispatch = new CombatHudStateHandler().handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":0.42,"stamina_percent":0.85,
             "combat_active":true,
             "derived":{"flying":true,"phasing":false,"tribulation_locked":false}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        CombatHudState state = CombatHudStateStore.snapshot();
        assertEquals(0.42f, state.qiPercent(), 1e-6);
        assertEquals(0.85f, state.staminaPercent(), 1e-6);
        assertTrue(state.combatActive(), "权威帧的 combat_active=true 必须进入战斗态");
        assertEquals(state, CombatHudStateStore.authoritativeSnapshot());
        assertTrue(state.derived().flying());
        assertFalse(state.derived().phasing());
    }

    @Test
    void rejectsMissingDerivedField() {
        ServerDataDispatch dispatch = new CombatHudStateHandler().handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":0.5,"stamina_percent":0.5,
             "combat_active":false}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("required fields"));
    }

    @Test
    void rejectsOutOfRangePercent() {
        ServerDataDispatch dispatch = new CombatHudStateHandler().handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":1.5,"stamina_percent":0.5,
             "combat_active":false,
             "derived":{"flying":false,"phasing":false,"tribulation_locked":false}}
            """));

        assertFalse(dispatch.handled());
    }

    @Test
    void appliesAuthoritativeOutOfCombatPayloadWithoutActivatingCombat() {
        ServerDataDispatch dispatch = new CombatHudStateHandler().handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":0.5,"stamina_percent":0.5,
             "combat_active":false,
             "derived":{"flying":false,"phasing":false,"tribulation_locked":false}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertFalse(CombatHudStateStore.snapshot().combatActive(),
            "普通 HUD 更新的 combat_active=false 不得伪造战斗态");
        assertTrue(CombatHudStateStore.snapshot().active(),
            "合法权威 HUD 帧仍必须激活 HUD 数值显示");
    }

    @Test
    void missingCombatActiveClearsPreviousAuthoritativeSnapshot() {
        CombatHudStateHandler handler = new CombatHudStateHandler();
        handler.handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":0.5,"stamina_percent":0.5,
             "combat_active":true,
             "derived":{"flying":false,"phasing":false,"tribulation_locked":false}}
            """));

        ServerDataDispatch dispatch = handler.handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":0.5,"stamina_percent":0.5,
             "derived":{"flying":false,"phasing":false,"tribulation_locked":false}}
            """));

        assertFalse(dispatch.handled());
        assertNull(CombatHudStateStore.authoritativeSnapshot(),
            "缺少 combat_active 的坏帧必须撤销旧权威快照，避免策略继续使用 stale 状态");
        assertFalse(CombatHudStateStore.snapshot().active(),
            "缺少 combat_active 的坏帧必须同时关闭 HUD snapshot");
    }

    @Test
    void invalidDerivedFlagClearsPreviousAuthoritativeSnapshot() {
        CombatHudStateHandler handler = new CombatHudStateHandler();
        handler.handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":0.5,"stamina_percent":0.5,
             "combat_active":true,
             "derived":{"flying":false,"phasing":false,"tribulation_locked":false}}
            """));

        ServerDataDispatch dispatch = handler.handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":0.5,"stamina_percent":0.5,
             "combat_active":true,
             "derived":{"flying":0,"phasing":false,"tribulation_locked":false}}
            """));

        assertFalse(dispatch.handled());
        assertNull(CombatHudStateStore.authoritativeSnapshot(),
            "derived flag 类型错误必须撤销旧权威快照，避免错误状态放行社交策略");
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
