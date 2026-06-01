package com.bong.client.hud.war;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-offscreen-war-v1 P9：FactionWarHudPlanner 饱和测试。
 *
 * <p>headless 可跑（无 MC 依赖）。覆盖：
 * <ul>
 *   <li>空 store → 零命令</li>
 *   <li>skirmish → 双色条 + 阶段文字</li>
 *   <li>settling + outcome → 结局结语</li>
 *   <li>aftermath → 无结局结语（store 已 clear）</li>
 *   <li>四 phase 文字映射 pin</li>
 *   <li>reframe b：文字不含具名宗门</li>
 *   <li>坐标不越界（x >= 0, y >= 0）</li>
 * </ul>
 */
class FactionWarHudPlannerTest {

    @AfterEach
    void tearDown() {
        FactionWarHudStore.resetForTests();
    }

    // ─────────── A. store 空 → 零命令 ─────────────────────────────────────────

    @Test
    void emptyStoreProducesNoCommands() {
        List<HudRenderCommand> cmds = FactionWarHudPlanner.buildCommands(800, 600, 0L);
        assertTrue(cmds.isEmpty(),
            "期望空 store 无渲染命令（因无 active 战事），实际 " + cmds.size() + " 条");
    }

    @Test
    void zeroDimensionsProducesNoCommands() {
        FactionWarHudStore.replace(skirmishState("残灰谷"));
        assertTrue(FactionWarHudPlanner.buildCommands(0, 600, 0L).isEmpty(),
            "期望 screenWidth=0 时无命令");
        assertTrue(FactionWarHudPlanner.buildCommands(800, 0, 0L).isEmpty(),
            "期望 screenHeight=0 时无命令");
    }

    // ─────────── B. skirmish → 双色条 + 阶段文字 ─────────────────────────────

    @Test
    void skirmishDrawsTwoRectsAndPhaseText() {
        FactionWarHudStore.replace(skirmishState("残灰谷"));
        List<HudRenderCommand> cmds = FactionWarHudPlanner.buildCommands(800, 600, 0L);

        assertFalse(cmds.isEmpty(), "期望 skirmish 阶段输出非空命令");

        long rectCount = cmds.stream().filter(c -> !c.isText()).count();
        assertTrue(rectCount >= 2,
            "期望至少 2 个 rect（甲方/乙方双色条），实际 " + rectCount);

        boolean hasPhaseText = cmds.stream().anyMatch(c -> c.isText() && c.text().contains("交锋"));
        assertTrue(hasPhaseText,
            "期望 skirmish 阶段文字含「交锋」，实际命令: " + cmds);
    }

    @Test
    void skirmishUsesCorrectLayer() {
        FactionWarHudStore.replace(skirmishState("残灰谷"));
        List<HudRenderCommand> cmds = FactionWarHudPlanner.buildCommands(800, 600, 0L);
        boolean allFactionWarLayer = cmds.stream().allMatch(c -> c.layer() == HudRenderLayer.FACTION_WAR);
        assertTrue(allFactionWarLayer,
            "期望所有命令使用 HudRenderLayer.FACTION_WAR，实际有非法 layer");
    }

    // ─────────── C. settling + outcome → 结局结语 ─────────────────────────────

    @Test
    void settlingWithOutcomeShowsOutcomeText() {
        FactionWarHudStore.replace(settlingWithOutcomeState("残灰谷"));
        List<HudRenderCommand> cmds = FactionWarHudPlanner.buildCommands(800, 600, 0L);

        boolean hasOutcomeText = cmds.stream().anyMatch(c ->
            c.isText() && c.text().contains("占上风"));
        assertTrue(hasOutcomeText,
            "期望 settling+outcome 阶段显示「占上风」结语，实际命令: " + cmds);

        boolean hasPhaseText = cmds.stream().anyMatch(c -> c.isText() && c.text().contains("落定"));
        assertTrue(hasPhaseText, "期望落定阶段文字存在");
    }

    @Test
    void settlingWithoutOutcomeNoOutcomeText() {
        // settling 但 winnerGroup=-1（无 outcome）
        FactionWarHudStore.replace(new FactionWarHudStore.State(
            "1", "残灰谷", "残灰谷一带散修", "settling",
            List.of(0, 1), 1, 0, 0, 0, -1, -1
        ));
        List<HudRenderCommand> cmds = FactionWarHudPlanner.buildCommands(800, 600, 0L);
        boolean hasOutcomeText = cmds.stream().anyMatch(c ->
            c.isText() && c.text().contains("占上风"));
        assertFalse(hasOutcomeText,
            "期望无 outcome 时不显示「占上风」结语");
    }

    // ─────────── D. aftermath → clear 后 store 为空 → 零命令 ─────────────────

    @Test
    void aftermathStateIsNotActiveByDefault() {
        // aftermath 阶段 active()=false → planner 不渲染
        FactionWarHudStore.State s = new FactionWarHudStore.State(
            "1", "残灰谷", "残灰谷一带散修", "aftermath",
            List.of(0, 1), 0, 0, 0, 0, 0, 1
        );
        assertFalse(s.active(), "期望 aftermath 阶段 active()==false");
        FactionWarHudStore.replace(s);
        List<HudRenderCommand> cmds = FactionWarHudPlanner.buildCommands(800, 600, 0L);
        assertTrue(cmds.isEmpty(),
            "期望 aftermath（active=false）时 planner 零命令，实际 " + cmds.size());
    }

    // ─────────── E. 四 phase 文字映射 pin ────────────────────────────────────

    @Test
    void phaseChineseLabelMapsAllFourVariants() {
        assertEquals("涌动", FactionWarHudPlanner.phaseChineseLabel("emerging"));
        assertEquals("交锋", FactionWarHudPlanner.phaseChineseLabel("skirmish"));
        assertEquals("落定", FactionWarHudPlanner.phaseChineseLabel("settling"));
        assertEquals("余波", FactionWarHudPlanner.phaseChineseLabel("aftermath"));
    }

    @Test
    void phaseChineseLabelFallback() {
        assertEquals("未知", FactionWarHudPlanner.phaseChineseLabel("unknown_phase"));
    }

    // ─────────── F. reframe b：文字不含具名宗门 ──────────────────────────────

    @Test
    void noNamedSectInText() {
        FactionWarHudStore.replace(settlingWithOutcomeState("残灰谷"));
        List<HudRenderCommand> cmds = FactionWarHudPlanner.buildCommands(800, 600, 0L);
        for (HudRenderCommand c : cmds) {
            if (!c.isText()) continue;
            String text = c.text();
            // reframe b：不含「青云」「盟」「宗主」等具名宗门字样
            // 注意：使用分组 (a|b) 而非字符类 [abc]（后者会误匹配单字）
            assertFalse(text.matches(".*(青云|盟|宗主|门派|宗门).*"),
                "期望文字不含具名宗门字样（reframe b），实际: " + text);
        }
    }

    @Test
    void regionDescriptorContainsSanxiu() {
        FactionWarHudStore.replace(settlingWithOutcomeState("残灰谷"));
        List<HudRenderCommand> cmds = FactionWarHudPlanner.buildCommands(800, 600, 0L);
        boolean hasSanxiu = cmds.stream().anyMatch(c ->
            c.isText() && c.text().contains("散修"));
        assertTrue(hasSanxiu,
            "期望结局文字含「散修」（reframe b：匿名散修描述符），实际命令: " + cmds);
    }

    // ─────────── G. 坐标不越界 ──────────────────────────────────────────────

    @Test
    void commandCoordinatesInBounds() {
        FactionWarHudStore.replace(skirmishState("残灰谷"));
        int sw = 800, sh = 600;
        List<HudRenderCommand> cmds = FactionWarHudPlanner.buildCommands(sw, sh, 0L);
        for (HudRenderCommand c : cmds) {
            assertTrue(c.x() >= 0,
                "期望 x >= 0（不越界），实际 x=" + c.x() + " cmd=" + c);
            assertTrue(c.y() >= 0,
                "期望 y >= 0（不越界），实际 y=" + c.y() + " cmd=" + c);
        }
    }

    // ─────────── H. 与 tribulation broadcast 不重叠 ─────────────────────────

    @Test
    void topMarginDoesNotOverlapTribulationBroadcast() {
        // TribulationBroadcastHudPlanner.TOP_MARGIN = 28, BAR_HEIGHT=18 → 最大 y=28+18=46
        // FactionWarHudPlanner.TOP_MARGIN 应 > 46（至少 47+），spec 设 50
        assertTrue(FactionWarHudPlanner.TOP_MARGIN >= 47,
            "期望 FactionWarHudPlanner.TOP_MARGIN >= 47（不压 tribulation broadcast），实际 "
                + FactionWarHudPlanner.TOP_MARGIN);
    }

    // ─────────── 辅助 fixtures ────────────────────────────────────────────────

    private static FactionWarHudStore.State skirmishState(String zone) {
        return new FactionWarHudStore.State(
            "1", zone, zone + "一带散修", "skirmish",
            List.of(0, 1), 2, 0, 1, 0, -1, -1
        );
    }

    private static FactionWarHudStore.State settlingWithOutcomeState(String zone) {
        return new FactionWarHudStore.State(
            "1", zone, zone + "一带散修", "settling",
            List.of(0, 1), 2, 0, 1, 0, 0, 1
        );
    }
}
