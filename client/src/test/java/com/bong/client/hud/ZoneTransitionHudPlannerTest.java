package com.bong.client.hud;

import com.bong.client.state.ZoneState;
import org.junit.jupiter.api.Test;
import java.util.Set;
import static org.junit.jupiter.api.Assertions.*;

class ZoneTransitionHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer WIDTH = text -> text.length() * 6;

    @Test
    void transitionFadesThenLeavesNoPersistentZonePanel() {
        ZoneState state = ZoneState.create("blood_valley", "血谷", 0.42, 3, 1_000L);
        var start = ZoneTransitionHudPlanner.buildCommands(state, 1_000L, WIDTH, 320, 240);
        var fading = ZoneTransitionHudPlanner.buildCommands(state, 3_500L, WIDTH, 320, 240);
        assertEquals("— 血谷 —", start.get(0).text());
        assertTrue((fading.get(0).color() >>> 24) < (start.get(0).color() >>> 24));
        assertTrue(ZoneTransitionHudPlanner.buildCommands(state, 4_000L, WIDTH, 320, 240).isEmpty(),
            "切区结束后必须完全隐藏，不能留下灵气/危险常驻文字");
    }

    @Test
    void dimensionBlackoutExpiresAndDisconnectClearsTitle() {
        ZoneState state = ZoneState.create("tsy_void", "天水窑", 0.3, 2,
            "dimension_transition", Set.of(), 1_000L);
        assertTrue(ZoneTransitionHudPlanner.buildCommands(state, 1_100L, WIDTH, 320, 240)
            .stream().anyMatch(HudRenderCommand::isScreenTint));
        assertTrue(ZoneTransitionHudPlanner.buildCommands(state, 1_500L, WIDTH, 320, 240)
            .stream().noneMatch(HudRenderCommand::isScreenTint));
        assertTrue(ZoneTransitionHudPlanner.buildCommands(ZoneState.empty(), 1_100L, WIDTH, 320, 240).isEmpty());
    }
}
