package com.bong.client.hud;

import com.bong.client.tsy.ExtractState;
import com.bong.client.tsy.RiftPortalView;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertTrue;

public class ExtractProgressHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @Test
    void extractingStateBuildsProgressBar() {
        ExtractState state = new ExtractState(
            List.of(),
            42L,
            "main_rift",
            40,
            160,
            true,
            "",
            0xFFFFFFFF,
            0L,
            "",
            0L,
            0,
            0L,
            0,
            0L
        );

        List<HudRenderCommand> commands = ExtractProgressHudPlanner.buildCommands(state, FIXED_WIDTH, 960, 540, 1000L);

        assertTrue(commands.stream().anyMatch(HudRenderCommand::isRect));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("撤离中")));
    }

    @Test
    void collapseStateBuildsCountdownTint() {
        ExtractState state = new ExtractState(
            List.of(new RiftPortalView(42L, "collapse_tear", "exit", "tsy_lingxu_01", 1, 2, 3, 2.0, 60, null)),
            null,
            "",
            0,
            0,
            false,
            "",
            0xFFFFFFFF,
            0L,
            "tsy_lingxu_01",
            1000L,
            600,
            0L,
            0,
            1000L
        );

        List<HudRenderCommand> commands = ExtractProgressHudPlanner.buildCommands(state, FIXED_WIDTH, 960, 540, 1000L);

        // plan-tsy-raceout-v1 P0：红色屏幕 tint + race-out 紧迫文案 + 撞墙换裂口提示。
        assertTrue(commands.stream().anyMatch(HudRenderCommand::isScreenTint),
            "race-out 期间应有红色屏幕 tint 警告，实际 commands=" + commands);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("race-out")),
            "HUD 文案需含 race-out 关键词（worldview §十六.六），实际 commands=" + commands);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("化死域")),
            "HUD 应提示后果（化死域），实际 commands=" + commands);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("已占即换")),
            "HUD 应提示 Q-RC4 撞墙换裂口规则，实际 commands=" + commands);
    }

    @Test
    void whiteFlashStateBuildsScreenTint() {
        ExtractState state = new ExtractState(
            List.of(),
            null,
            "",
            0,
            0,
            false,
            "已撤出：tsy_lingxu_01",
            0xFF80FF80,
            1500L,
            "",
            0L,
            0,
            1500L,
            0xCCFFFFFF,
            1000L
        );

        List<HudRenderCommand> commands = ExtractProgressHudPlanner.buildCommands(state, FIXED_WIDTH, 960, 540, 1000L);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isScreenTint() && cmd.color() == 0xCCFFFFFF));
    }
}
