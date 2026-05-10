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

        // plan-tsy-raceout-v2 P0/P1：红色 tint + 大屏 race-out + 本族裂口列表。
        assertTrue(commands.stream().anyMatch(HudRenderCommand::isScreenTint),
            "race-out 期间应有红色屏幕 tint 警告，实际 commands=" + commands);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("RACE-OUT")),
            "HUD 文案需含 race-out 关键词（worldview §十六.六），实际 commands=" + commands);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isScaledText() && cmd.text().equals("30")),
            "HUD 中央应有向上取整的大号秒数，实际 commands=" + commands);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("化死域")),
            "HUD 应提示后果（化死域），实际 commands=" + commands);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("已占即换")),
            "HUD 应提示 Q-RC4 撞墙换裂口规则，实际 commands=" + commands);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("本族裂口")),
            "HUD 应列出本族塌缩裂口，实际 commands=" + commands);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("距 4 格")),
            "裂口列表应显示玩家到 CollapseTear 的距离，实际 commands=" + commands);
    }

    @Test
    void collapseRiftListFiltersFamilyKindDirectionAndSortsNearestFive() {
        ExtractState state = new ExtractState(
            List.of(
                new RiftPortalView(1L, "collapse_tear", "exit", "tsy_lingxu_01", 5, 0, 0, 2.0, 60, null),
                new RiftPortalView(2L, "collapse_tear", "exit", "tsy_lingxu_01", 1, 0, 0, 2.0, 60, null),
                new RiftPortalView(3L, "collapse_tear", "exit", "tsy_lingxu_01", 3, 0, 0, 2.0, 60, null),
                new RiftPortalView(4L, "collapse_tear", "entry", "tsy_lingxu_01", 0, 0, 0, 2.0, 60, null),
                new RiftPortalView(5L, "deep_rift", "exit", "tsy_lingxu_01", 0, 0, 0, 2.0, 60, null),
                new RiftPortalView(6L, "collapse_tear", "exit", "tsy_other", 0, 0, 0, 2.0, 60, null),
                new RiftPortalView(7L, "collapse_tear", "exit", "tsy_lingxu_01", 2, 0, 0, 2.0, 60, null),
                new RiftPortalView(8L, "collapse_tear", "exit", "tsy_lingxu_01", 4, 0, 0, 2.0, 60, null),
                new RiftPortalView(9L, "collapse_tear", "exit", "tsy_lingxu_01", 6, 0, 0, 2.0, 60, null)
            ),
            7L,
            "collapse_tear",
            0,
            60,
            true,
            "",
            0xFFFFFFFF,
            0L,
            "tsy_lingxu_01",
            1000L,
            60,
            0L,
            0,
            1000L
        );

        List<HudRenderCommand> commands = ExtractProgressHudPlanner.buildCommands(state, FIXED_WIDTH, 960, 540, 1000L);
        List<String> riftLines = commands.stream()
            .filter(HudRenderCommand::isText)
            .map(HudRenderCommand::text)
            .filter(text -> text.contains("距 "))
            .toList();

        assertTrue(riftLines.size() == 5, "只应展示最近 5 个同 family exit CollapseTear，实际：" + riftLines);
        assertTrue(riftLines.get(0).contains("距 1 格"), "列表应按距离升序，实际：" + riftLines);
        assertTrue(riftLines.stream().anyMatch(text -> text.startsWith("×") && text.contains("距 2 格")),
            "当前正在撤离的裂口应显示已占标记，实际：" + riftLines);
        assertTrue(riftLines.stream().noneMatch(text -> text.contains("距 0 格")),
            "列表必须过滤跨 family / 非 exit / 非 CollapseTear，实际：" + riftLines);
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
