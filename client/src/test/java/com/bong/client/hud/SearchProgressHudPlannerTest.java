package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class SearchProgressHudPlannerTest {
    @Test
    void hiddenWhenIdle() {
        List<HudRenderCommand> cmds = SearchProgressHudPlanner.buildCommands(
            SearchHudState.idle(), 800, 600
        );
        assertTrue(cmds.isEmpty());
    }

    @Test
    void hiddenWhenStateNull() {
        List<HudRenderCommand> cmds = SearchProgressHudPlanner.buildCommands(null, 800, 600);
        assertTrue(cmds.isEmpty());
    }

    @Test
    void searchingDrawsTrackFillAndText() {
        // 80 ticks 总时长，已过 40 → 50%
        SearchHudState state = SearchHudState.searching("干尸", 80, 40);
        List<HudRenderCommand> cmds = SearchProgressHudPlanner.buildCommands(state, 800, 600);
        assertEquals(3, cmds.size(), "Expected track + fill + text");
        for (HudRenderCommand c : cmds) {
            assertEquals(HudRenderLayer.SEARCH_PROGRESS, c.layer());
        }
    }

    @Test
    void searchingZeroProgressDrawsTrackOnly() {
        SearchHudState state = SearchHudState.searching("干尸", 80, 0);
        List<HudRenderCommand> cmds = SearchProgressHudPlanner.buildCommands(state, 800, 600);
        // 0% 进度 → fill 0 → 不绘制 fill；只有 track + text
        assertEquals(2, cmds.size());
    }

    @Test
    void completedFlashShowsBarFullAndText() {
        SearchHudState state = SearchHudState.completed("石匣");
        List<HudRenderCommand> cmds = SearchProgressHudPlanner.buildCommands(state, 800, 600);
        assertEquals(2, cmds.size(), "Expected bar + completed text");
    }

    @Test
    void abortedFlashShowsTextOnly() {
        SearchHudState state = SearchHudState.aborted("石匣", SearchHudState.AbortReason.MOVED);
        List<HudRenderCommand> cmds = SearchProgressHudPlanner.buildCommands(state, 800, 600);
        assertEquals(1, cmds.size(), "Expected only abort text");
    }

    @Test
    void abortReasonLabelsLocalised() {
        assertEquals("位置偏移", SearchProgressHudPlanner.abortReasonLabel(SearchHudState.AbortReason.MOVED));
        assertEquals("进入战斗", SearchProgressHudPlanner.abortReasonLabel(SearchHudState.AbortReason.COMBAT));
        assertEquals("受击", SearchProgressHudPlanner.abortReasonLabel(SearchHudState.AbortReason.DAMAGED));
        assertEquals("已取消", SearchProgressHudPlanner.abortReasonLabel(SearchHudState.AbortReason.CANCELLED));
    }

    @Test
    void hiddenWhenScreenZero() {
        SearchHudState state = SearchHudState.searching("干尸", 80, 40);
        assertTrue(SearchProgressHudPlanner.buildCommands(state, 0, 600).isEmpty());
        assertTrue(SearchProgressHudPlanner.buildCommands(state, 800, 0).isEmpty());
    }

    @Test
    void searchHudStateRemainingSecondsClamps() {
        SearchHudState s = SearchHudState.searching("干尸", 80, 80);
        assertEquals(0, s.remainingSeconds());
        SearchHudState s2 = SearchHudState.searching("干尸", 80, 0);
        assertEquals(4, s2.remainingSeconds());
    }

    @Test
    void searchHudStateProgressRatioBoundedAtOne() {
        // elapsed > required（不可能但兜底）
        SearchHudState s = SearchHudState.searching("石匣", 100, 200);
        assertEquals(1.0f, s.progressRatio(), 0.001f);
    }

    @Test
    void searchHudStateNonSearchingPhasesReturnZero() {
        assertEquals(0f, SearchHudState.idle().progressRatio());
        assertEquals(0, SearchHudState.idle().remainingSeconds());
        assertEquals(0f, SearchHudState.completed("x").progressRatio());
        assertEquals(0f, SearchHudState.aborted("x", SearchHudState.AbortReason.MOVED).progressRatio());
    }
}
