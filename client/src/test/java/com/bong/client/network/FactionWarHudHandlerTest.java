package com.bong.client.network;

import com.bong.client.hud.war.FactionWarHudStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-offscreen-war-v1 P9：FactionWarHudHandler JSON round-trip + store 写入测试。
 *
 * <p>headless 可跑（无 MC 依赖）。覆盖：
 * <ul>
 *   <li>全字段 settling 正 round-trip（含 winner/loser Some）</li>
 *   <li>skirmish 无 outcome（winner_group absent = -1）</li>
 *   <li>aftermath → clear()</li>
 *   <li>缺字段 graceful（不 NPE）</li>
 *   <li>reframe b：regionDescriptor 含「一带散修」不含具名宗门</li>
 * </ul>
 */
public class FactionWarHudHandlerTest {

    private static final ServerDataRouter ROUTER = ServerDataRouter.createDefault();

    @AfterEach
    void tearDown() {
        FactionWarHudStore.resetForTests();
    }

    // ─────────── A. settling 全字段 round-trip ─────────────────────────────

    @Test
    void settlingFullFieldsRoundTrip() {
        String json = """
            {
              "v": 1,
              "type": "faction_war_state",
              "war_id": "7",
              "zone": "残灰谷",
              "region_descriptor": "残灰谷一带散修",
              "phase": "settling",
              "groups": [0, 1],
              "enlist_count": 2,
              "mercenary_count": 0,
              "intercept_count": 1,
              "spectate_count": 0,
              "winner_group": 0,
              "loser_group": 1,
              "at_tick": 10000
            }
            """;
        ServerDataRouter.RouteResult result = ROUTER.route(json, 0);
        assertTrue(result.isHandled(), "期望 faction_war_state 被 handler 处理，实际: " + result.logMessage());

        FactionWarHudStore.State state = FactionWarHudStore.snapshot();
        assertEquals("残灰谷", state.zone(),
            "期望 zone='残灰谷'，实际 " + state.zone());
        assertEquals("残灰谷一带散修", state.regionDescriptor(),
            "期望 regionDescriptor='残灰谷一带散修'，实际 " + state.regionDescriptor());
        assertEquals("settling", state.phase(),
            "期望 phase='settling'，实际 " + state.phase());
        assertEquals(List.of(0, 1), state.groups(),
            "期望 groups=[0,1]，实际 " + state.groups());
        assertEquals(2, state.enlistCount(),
            "期望 enlistCount=2，实际 " + state.enlistCount());
        assertEquals(0, state.winnerGroup() == -1 ? -1 : state.winnerGroup(),
            "期望 winnerGroup=0（Some(0)），实际 " + state.winnerGroup());
        assertEquals(1, state.loserGroup(),
            "期望 loserGroup=1，实际 " + state.loserGroup());
        assertTrue(state.hasOutcome(),
            "期望 settling+winner 有 outcome，实际 hasOutcome()=false");
    }

    // ─────────── B. skirmish 无 outcome ──────────────────────────────────────

    @Test
    void skirmishNoOutcomeWinnerIsAbsent() {
        String json = """
            {
              "v": 1,
              "type": "faction_war_state",
              "war_id": "3",
              "zone": "残灰谷",
              "region_descriptor": "残灰谷一带散修",
              "phase": "skirmish",
              "groups": [0, 1],
              "enlist_count": 1,
              "mercenary_count": 1,
              "intercept_count": 0,
              "spectate_count": 2,
              "at_tick": 5000
            }
            """;
        ServerDataRouter.RouteResult result = ROUTER.route(json, 0);
        assertTrue(result.isHandled(), result.logMessage());

        FactionWarHudStore.State state = FactionWarHudStore.snapshot();
        assertEquals("skirmish", state.phase());
        assertEquals(-1, state.winnerGroup(),
            "期望 skirmish 无 winner_group → -1（None），实际 " + state.winnerGroup());
        assertEquals(-1, state.loserGroup(),
            "期望 skirmish 无 loser_group → -1（None），实际 " + state.loserGroup());
        assertFalse(state.hasOutcome(), "期望 skirmish 无 outcome");
        assertTrue(state.active(), "期望 skirmish active()==true");
    }

    @Test
    void numericWarIdIsPreservedAsStoreString() {
        String json = """
            {
              "v": 1,
              "type": "faction_war_state",
              "war_id": 42,
              "zone": "残灰谷",
              "region_descriptor": "残灰谷一带散修",
              "phase": "skirmish",
              "groups": [0, 1],
              "enlist_count": 1,
              "mercenary_count": 1,
              "intercept_count": 0,
              "spectate_count": 2,
              "at_tick": 5000
            }
            """;
        ServerDataRouter.RouteResult result = ROUTER.route(json, 0);
        assertTrue(result.isHandled(), result.logMessage());

        assertEquals("42", FactionWarHudStore.snapshot().warId(),
            "proto bridge 会把 uint64 war_id 归一成 JSON number，handler 仍应保留 ID 文本");
    }

    // ─────────── C. aftermath → clear ────────────────────────────────────────

    @Test
    void aftermathClearsStore() {
        // 先设一个 skirmish
        FactionWarHudStore.replace(new FactionWarHudStore.State(
            "1", "残灰谷", "残灰谷一带散修", "skirmish",
            List.of(0, 1), 1, 0, 0, 0, -1, -1
        ));
        assertFalse(FactionWarHudStore.snapshot().phase().isEmpty());

        String json = """
            {
              "v": 1,
              "type": "faction_war_state",
              "zone": "残灰谷",
              "region_descriptor": "残灰谷一带散修",
              "phase": "aftermath",
              "groups": [0, 1],
              "enlist_count": 0,
              "mercenary_count": 0,
              "intercept_count": 0,
              "spectate_count": 0,
              "winner_group": 0,
              "loser_group": 1,
              "at_tick": 20000
            }
            """;
        ServerDataRouter.RouteResult result = ROUTER.route(json, 0);
        assertTrue(result.isHandled(), result.logMessage());

        FactionWarHudStore.State state = FactionWarHudStore.snapshot();
        assertTrue(state.phase().isEmpty(),
            "期望 aftermath 后 store 被 clear（phase 为空），实际 phase='" + state.phase() + "'");
    }

    // ─────────── D. 空 phase → clear ─────────────────────────────────────────

    @Test
    void emptyPhaseClearsStore() {
        FactionWarHudStore.replace(new FactionWarHudStore.State(
            "1", "残灰谷", "残灰谷一带散修", "skirmish",
            List.of(0, 1), 1, 0, 0, 0, -1, -1
        ));

        String json = """
            {
              "v": 1,
              "type": "faction_war_state",
              "zone": "残灰谷",
              "region_descriptor": "残灰谷一带散修",
              "phase": "",
              "groups": [],
              "enlist_count": 0,
              "mercenary_count": 0,
              "intercept_count": 0,
              "spectate_count": 0,
              "at_tick": 30000
            }
            """;
        ROUTER.route(json, 0);
        assertTrue(FactionWarHudStore.snapshot().phase().isEmpty(),
            "期望空 phase 后 store 被 clear");
    }

    // ─────────── E. 缺字段 graceful ──────────────────────────────────────────

    @Test
    void missingOptionalFieldsUseDefaults() {
        // 仅 required 字段（phase=skirmish, zone, region_descriptor, groups 基础字段）
        String json = """
            {
              "v": 1,
              "type": "faction_war_state",
              "phase": "skirmish",
              "zone": "残灰谷",
              "region_descriptor": "残灰谷一带散修",
              "groups": [],
              "enlist_count": 0,
              "mercenary_count": 0,
              "intercept_count": 0,
              "spectate_count": 0,
              "at_tick": 1000
            }
            """;
        ServerDataRouter.RouteResult result = ROUTER.route(json, 0);
        assertTrue(result.isHandled(), "期望缺可选字段时仍 handled，实际: " + result.logMessage());
        assertEquals(-1, FactionWarHudStore.snapshot().winnerGroup(),
            "期望缺 winner_group 时默认 -1（None）");
    }

    // ─────────── F. reframe b：regionDescriptor 不含具名宗门 ─────────────────

    @Test
    void regionDescriptorContainsSanxiuNotNamedSect() {
        String json = """
            {
              "v": 1,
              "type": "faction_war_state",
              "war_id": "1",
              "zone": "残灰谷",
              "region_descriptor": "残灰谷一带散修",
              "phase": "skirmish",
              "groups": [0, 1],
              "enlist_count": 0,
              "mercenary_count": 0,
              "intercept_count": 0,
              "spectate_count": 0,
              "at_tick": 100
            }
            """;
        ROUTER.route(json, 0);
        String rd = FactionWarHudStore.snapshot().regionDescriptor();
        assertTrue(rd.contains("散修"),
            "期望 regionDescriptor 含「散修」（reframe b），实际: " + rd);
        assertFalse(rd.matches(".*[青云盟宗主].*"),
            "期望 regionDescriptor 不含具名宗门字样（reframe b），实际: " + rd);
    }
}
