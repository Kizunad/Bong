package com.bong.client.hud;

import com.bong.client.combat.handler.ZhenmaiHudServerDataHandler;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 震脉 5 招 + 盾格挡 HUD planner 的饱和化测试。
 *
 * <p>覆盖：① happy path（每维度激活渲染）② 条件隐藏（未激活/过期/空快照不渲染）
 * ③ 边界（screen 非法、duration 边界、per-dimension 不互覆）④ handler→store→planner 全链。
 */
class ZhenmaiHudPlannerTest {

    private static final int W = 960;
    private static final int H = 540;

    private ZhenmaiHudServerDataHandler handler;

    @BeforeEach
    void setUp() {
        handler = new ZhenmaiHudServerDataHandler();
        ZhenmaiHudStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        ZhenmaiHudStateStore.resetForTests();
    }

    private static ServerDataEnvelope parse(String json) {
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(r.isSuccess(), () -> "parse failed: " + r.errorMessage());
        return r.envelope();
    }

    private static boolean hasLayer(List<HudRenderCommand> cmds, HudRenderLayer layer) {
        return cmds.stream().anyMatch(c -> c.layer() == layer);
    }

    // ─── 条件隐藏：空快照 / 非法屏幕 ───────────────────────────────

    @Test
    void emptySnapshotEmitsNothing() {
        long now = 1_000L;
        assertTrue(
            ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.Snapshot.NONE, now, W, H).isEmpty(),
            "全维度未激活时不应 emit 任何命令（绝不常驻）"
        );
    }

    @Test
    void nullSnapshotEmitsNothing() {
        assertTrue(ZhenmaiHudPlanner.buildCommands(null, 1_000L, W, H).isEmpty(),
            "null 快照应安全返回空");
    }

    @Test
    void zeroScreenEmitsNothing() {
        long now = 1_000L;
        ZhenmaiHudStateStore.flashParry(now, 450L);
        assertTrue(ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, 0, H).isEmpty(),
            "screenWidth<=0 应返回空");
        assertTrue(ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, 0).isEmpty(),
            "screenHeight<=0 应返回空");
    }

    // ─── parry：瞬态中心环高亮 ─────────────────────────────────────

    @Test
    void parryActiveShowsCenterRing() {
        long now = 1_000L;
        ZhenmaiHudStateStore.flashParry(now, 450L);
        List<HudRenderCommand> cmds = ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H);
        assertTrue(hasLayer(cmds, HudRenderLayer.ZHENMAI_PARRY), "弹反激活应产 ZHENMAI_PARRY 命令");
        // 中心环由 4 段 rect 构成
        assertEquals(4, cmds.stream().filter(c -> c.layer() == HudRenderLayer.ZHENMAI_PARRY && c.isRect()).count(),
            "中心环应为 4 段 rect（菱形环）");
    }

    @Test
    void parryExpiredHidesRing() {
        long start = 1_000L;
        ZhenmaiHudStateStore.flashParry(start, 450L);
        long afterExpiry = start + 451L;
        List<HudRenderCommand> cmds =
            ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(afterExpiry), afterExpiry, W, H);
        assertFalse(hasLayer(cmds, HudRenderLayer.ZHENMAI_PARRY),
            "弹反瞬态过期后应完全隐藏（不常驻）");
    }

    // ─── neutralize：去毒提示含 contam 量 ──────────────────────────

    @Test
    void neutralizeActiveShowsRemovedPercentAndMeridian() {
        long now = 2_000L;
        ZhenmaiHudStateStore.flashNeutralize(3.5f, "Lung", now, 1_400L);
        List<HudRenderCommand> cmds = ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H);
        assertTrue(
            cmds.stream().anyMatch(c -> c.layer() == HudRenderLayer.ZHENMAI_NEUTRALIZE
                && c.isText() && c.text().contains("Lung") && c.text().contains("3.5")),
            "中和应显示经脉名 + 去除量；实际=" + cmds
        );
    }

    @Test
    void neutralizeExpiredHides() {
        long now = 2_000L;
        ZhenmaiHudStateStore.flashNeutralize(3.5f, "Lung", now, 1_400L);
        long after = now + 1_500L;
        assertFalse(
            hasLayer(ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(after), after, W, H),
                HudRenderLayer.ZHENMAI_NEUTRALIZE),
            "中和提示过期后隐藏"
        );
    }

    // ─── multipoint：激活期反震计数 ────────────────────────────────

    @Test
    void multipointActiveShowsRemainingCount() {
        long now = 3_000L;
        ZhenmaiHudStateStore.setMultipoint(5, now, 5_000L);
        List<HudRenderCommand> cmds = ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H);
        assertTrue(
            cmds.stream().anyMatch(c -> c.layer() == HudRenderLayer.ZHENMAI_MULTIPOINT
                && c.isText() && c.text().contains("5")),
            "多点反震应显示剩余点数 5；实际=" + cmds
        );
    }

    @Test
    void multipointExpiredHides() {
        long now = 3_000L;
        ZhenmaiHudStateStore.setMultipoint(5, now, 5_000L);
        long after = now + 5_001L;
        assertFalse(
            hasLayer(ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(after), after, W, H),
                HudRenderLayer.ZHENMAI_MULTIPOINT),
            "多点反震 duration 结束后隐藏（不常驻）"
        );
    }

    // ─── harden：护脉减伤微条 ──────────────────────────────────────

    @Test
    void hardenActiveShowsReductionBarAndPercent() {
        long now = 4_000L;
        ZhenmaiHudStateStore.setHarden(0.65f, "Lung", now, 20_000L);
        List<HudRenderCommand> cmds = ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H);
        assertTrue(hasLayer(cmds, HudRenderLayer.ZHENMAI_HARDEN), "护脉激活应产 ZHENMAI_HARDEN");
        assertTrue(
            cmds.stream().anyMatch(c -> c.layer() == HudRenderLayer.ZHENMAI_HARDEN && c.isRect()),
            "护脉应有减伤微条 rect"
        );
        assertTrue(
            cmds.stream().anyMatch(c -> c.layer() == HudRenderLayer.ZHENMAI_HARDEN
                && c.isText() && c.text().contains("65")),
            "护脉应显示减伤 65%；实际=" + cmds
        );
    }

    @Test
    void hardenReductionClampedToUnitInterval() {
        long now = 4_000L;
        // 超过 1.0 的非法 reduction 应被 clamp（填充不溢出，仍 active）
        ZhenmaiHudStateStore.setHarden(5.0f, "Lung", now, 20_000L);
        List<HudRenderCommand> cmds = ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H);
        assertTrue(
            cmds.stream().filter(c -> c.layer() == HudRenderLayer.ZHENMAI_HARDEN && c.isRect())
                .allMatch(c -> c.width() <= ZhenmaiHudPlanner.BAR_WIDTH),
            "减伤条填充不应超过 BAR_WIDTH（reduction 已 clamp 到 [0,1]）"
        );
    }

    @Test
    void hardenExpiredHides() {
        long now = 4_000L;
        ZhenmaiHudStateStore.setHarden(0.65f, "Lung", now, 20_000L);
        long after = now + 20_001L;
        assertFalse(
            hasLayer(ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(after), after, W, H),
                HudRenderLayer.ZHENMAI_HARDEN),
            "护脉 buff 结束后隐藏"
        );
    }

    // ─── sever_chain：断脉标记 + 增幅倒计时 ────────────────────────

    @Test
    void severActiveShowsMeridianMarkerAndCountdownBar() {
        long now = 5_000L;
        ZhenmaiHudStateStore.setSever("Heart", 1.5f, now, 60_000L);
        List<HudRenderCommand> cmds = ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H);
        assertTrue(
            cmds.stream().anyMatch(c -> c.layer() == HudRenderLayer.ZHENMAI_SEVER
                && c.isText() && c.text().contains("Heart")),
            "断链应显示断掉的经脉 Heart；实际=" + cmds
        );
        assertTrue(
            cmds.stream().anyMatch(c -> c.layer() == HudRenderLayer.ZHENMAI_SEVER && c.isRect()),
            "断链应有增幅倒计时微条"
        );
    }

    @Test
    void severCountdownShrinksOverTime() {
        long start = 5_000L;
        ZhenmaiHudStateStore.setSever("Heart", 1.5f, start, 60_000L);
        int fullWidth = filledSeverBar(start);
        int halfWidth = filledSeverBar(start + 30_000L);
        assertTrue(halfWidth < fullWidth,
            "增幅窗口剩余越少倒计时条越短；full=" + fullWidth + " half=" + halfWidth);
    }

    private int filledSeverBar(long now) {
        List<HudRenderCommand> cmds = ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H);
        // 倒计时填充条是第二个 rect（bg 之后），取最大宽度的 rect 为 bg、较小为 filled。
        return cmds.stream()
            .filter(c -> c.layer() == HudRenderLayer.ZHENMAI_SEVER && c.isRect())
            .mapToInt(HudRenderCommand::width)
            .min()
            .orElse(0);
    }

    @Test
    void severExpiredHides() {
        long now = 5_000L;
        ZhenmaiHudStateStore.setSever("Heart", 1.5f, now, 60_000L);
        long after = now + 60_001L;
        assertFalse(
            hasLayer(ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(after), after, W, H),
                HudRenderLayer.ZHENMAI_SEVER),
            "增幅窗口结束后断脉标记隐藏"
        );
    }

    // ─── shield_block：瞬态盾弧 ────────────────────────────────────

    @Test
    void shieldBlockActiveShowsArc() {
        long now = 6_000L;
        ZhenmaiHudStateStore.flashShieldBlock(now, 350L);
        assertTrue(
            hasLayer(ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H),
                HudRenderLayer.SHIELD_BLOCK),
            "盾格挡命中瞬间应产 SHIELD_BLOCK 弧光"
        );
    }

    @Test
    void shieldBlockExpiredHides() {
        long now = 6_000L;
        ZhenmaiHudStateStore.flashShieldBlock(now, 350L);
        long after = now + 351L;
        assertFalse(
            hasLayer(ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(after), after, W, H),
                HudRenderLayer.SHIELD_BLOCK),
            "盾弧瞬态过期后隐藏"
        );
    }

    // ─── per-dimension 不互覆 ──────────────────────────────────────

    @Test
    void multiplePersistentDimensionsCoexist() {
        long now = 7_000L;
        ZhenmaiHudStateStore.setHarden(0.5f, "Lung", now, 20_000L);
        ZhenmaiHudStateStore.setMultipoint(4, now, 5_000L);
        ZhenmaiHudStateStore.setSever("Heart", 1.2f, now, 60_000L);
        List<HudRenderCommand> cmds = ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H);
        assertTrue(hasLayer(cmds, HudRenderLayer.ZHENMAI_HARDEN), "harden 维度应在");
        assertTrue(hasLayer(cmds, HudRenderLayer.ZHENMAI_MULTIPOINT), "multipoint 维度应在");
        assertTrue(hasLayer(cmds, HudRenderLayer.ZHENMAI_SEVER), "sever 维度应在");
    }

    @Test
    void updatingOneDimensionDoesNotClearOthers() {
        long now = 7_000L;
        ZhenmaiHudStateStore.setHarden(0.5f, "Lung", now, 20_000L);
        // 之后只更新 parry，不应清掉 harden
        ZhenmaiHudStateStore.flashParry(now, 450L);
        List<HudRenderCommand> cmds = ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H);
        assertTrue(hasLayer(cmds, HudRenderLayer.ZHENMAI_HARDEN), "更新 parry 不应清掉 harden（per-dimension merge）");
        assertTrue(hasLayer(cmds, HudRenderLayer.ZHENMAI_PARRY), "parry 维度也应在");
    }

    // ─── handler → store → planner 全链 ───────────────────────────

    @Test
    void handlerParryToStoreToPlanner() {
        handler.handle(parse("{\"v\":1,\"type\":\"zhenmai_hud\",\"skill_id\":\"parry\",\"tick\":42}"));
        long now = System.currentTimeMillis();
        assertTrue(
            hasLayer(ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H),
                HudRenderLayer.ZHENMAI_PARRY),
            "handler(parry)→store→planner 应产 ZHENMAI_PARRY"
        );
    }

    @Test
    void handlerHardenToStoreToPlannerCarriesReductionAndMeridian() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"zhenmai_hud\",\"skill_id\":\"harden\",\"meridian_id\":\"Lung\","
                + "\"damage_reduction\":0.65,\"duration_ms\":20000,\"tick\":7}"
        ));
        long now = System.currentTimeMillis();
        List<HudRenderCommand> cmds = ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H);
        assertTrue(
            cmds.stream().anyMatch(c -> c.layer() == HudRenderLayer.ZHENMAI_HARDEN
                && c.isText() && c.text().contains("Lung") && c.text().contains("65")),
            "handler(harden)→store→planner 应携带经脉名 + 减伤%；实际=" + cmds
        );
    }

    @Test
    void handlerSeverToStoreToPlanner() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"zhenmai_hud\",\"skill_id\":\"sever_chain\",\"meridian_id\":\"Heart\","
                + "\"k_drain\":1.5,\"duration_ms\":60000,\"tick\":9}"
        ));
        long now = System.currentTimeMillis();
        assertTrue(
            ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H).stream()
                .anyMatch(c -> c.layer() == HudRenderLayer.ZHENMAI_SEVER && c.isText() && c.text().contains("Heart")),
            "handler(sever_chain)→store→planner 应显示 Heart 断脉标记"
        );
    }

    @Test
    void handlerUnknownSkillIdIsNoOp() {
        var dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"zhenmai_hud\",\"skill_id\":\"bogus\",\"tick\":1}"
        ));
        assertFalse(dispatch.handled(), "未知 skill_id 应 no-op");
        long now = System.currentTimeMillis();
        assertTrue(
            ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H).isEmpty(),
            "未知 skill_id 不应写入任何维度"
        );
    }

    @Test
    void handlerMultipointMissingDurationFallsBackToDefault() {
        // 缺 duration_ms → 回退 DEFAULT_DURATION_MS（仍 active）
        handler.handle(parse(
            "{\"v\":1,\"type\":\"zhenmai_hud\",\"skill_id\":\"multipoint\",\"remaining_points\":3,\"tick\":1}"
        ));
        long now = System.currentTimeMillis();
        assertTrue(
            hasLayer(ZhenmaiHudPlanner.buildCommands(ZhenmaiHudStateStore.snapshot(now), now, W, H),
                HudRenderLayer.ZHENMAI_MULTIPOINT),
            "缺 duration_ms 应回退默认值并仍渲染"
        );
    }
}
