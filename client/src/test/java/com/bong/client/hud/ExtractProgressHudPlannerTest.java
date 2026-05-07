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
        // plan-tsy-raceout-v1 P0 — 3 秒红色 race-out 倒计时 banner + 大号秒数。
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
            60,
            0L,
            0,
            1000L
        );

        List<HudRenderCommand> commands = ExtractProgressHudPlanner.buildCommands(state, FIXED_WIDTH, 960, 540, 1000L);

        assertTrue(commands.stream().anyMatch(HudRenderCommand::isScreenTint),
            "race-out should overlay a red screen tint");
        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("RACE-OUT")),
            "race-out banner should be present"
        );
        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && "3s".equals(cmd.text())),
            "race-out should show 3-second whole-second countdown at start"
        );
    }

    @Test
    void collapseStateCountdownTicksDownToOne() {
        // race-out 3 秒撤离窗口的"3 → 2 → 1"取整规则：剩余 30 ticks 应显示 2s。
        ExtractState state = new ExtractState(
            List.of(),
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
            60,
            0L,
            0,
            1000L
        );

        // collapseRemainingTicks 用 (now - start)/50 ms => tick；这里 now = start + 1500ms
        // 等价剩余 ticks = 60 - 1500/50 = 60 - 30 = 30 ticks = 1.5s 向上取整 = 2s。
        List<HudRenderCommand> commands = ExtractProgressHudPlanner.buildCommands(state, FIXED_WIDTH, 960, 540, 2500L);

        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && "2s".equals(cmd.text())),
            "1.5s 实际剩余应取整显示为 2s（玩家看到清晰倒数）"
        );
    }

    @Test
    void collapseRiftListShowsUpToFiveNearestSorted() {
        // plan-tsy-raceout-v1 P1 / Q-RC2 — race-out 阶段全副本玩家可见塌缩裂口列表，按距离升序，最多 5 个。
        // 也只显示当前坍缩 family 的 collapse_tear；其它 family / 其它 kind / entry 方向应被过滤。
        java.util.List<RiftPortalView> portals = List.of(
            new RiftPortalView(1L, "collapse_tear", "exit", "tsy_lingxu_01", 100, 64, 0, 2.0, 60, null),
            new RiftPortalView(2L, "collapse_tear", "exit", "tsy_lingxu_01", 50, 64, 0, 2.0, 60, null),
            new RiftPortalView(3L, "collapse_tear", "exit", "tsy_lingxu_01", 10, 64, 0, 2.0, 60, null),
            new RiftPortalView(4L, "collapse_tear", "exit", "tsy_lingxu_01", 200, 64, 0, 2.0, 60, null),
            new RiftPortalView(5L, "collapse_tear", "exit", "tsy_lingxu_01", 30, 64, 0, 2.0, 60, null),
            new RiftPortalView(6L, "collapse_tear", "exit", "tsy_lingxu_01", 5, 64, 0, 2.0, 60, null),
            // 应被过滤：标准 main_rift / 别族 / entry 方向
            new RiftPortalView(7L, "main_rift", "exit", "tsy_lingxu_01", 0, 64, 0, 2.0, 60, null),
            new RiftPortalView(8L, "collapse_tear", "exit", "other_family", 1, 64, 0, 2.0, 60, null),
            new RiftPortalView(9L, "collapse_tear", "entry", "tsy_lingxu_01", 1, 64, 0, 2.0, 60, null)
        );
        ExtractState state = new ExtractState(
            portals, null, "", 0, 0, false, "", 0xFFFFFFFF, 0L,
            "tsy_lingxu_01", 1000L, 60, 0L, 0, 1000L
        );

        java.util.List<HudRenderCommand> out = new java.util.ArrayList<>();
        ExtractProgressHudPlanner.appendCollapseRiftListWithPlayerPos(
            out, state, FIXED_WIDTH, 960, 540, new net.minecraft.util.math.Vec3d(0, 64, 0)
        );

        // 标题 panel 总在
        assertTrue(out.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("塌缩裂口")));
        // 最近 5 个：6(5m), 3(10m), 5(30m), 2(50m), 1(100m)。第 6 个 (id=4, 200m) 不应出现。
        long lineCount = out.stream().filter(cmd -> cmd.isText() && cmd.text().startsWith("#")).count();
        org.junit.jupiter.api.Assertions.assertEquals(5L, lineCount, "应只列前 5 个最近裂口，实际 " + lineCount);
        // 验证第一行是最近的（id=6, 距离 5m）
        assertTrue(
            out.stream().anyMatch(cmd -> cmd.isText() && "#1  5m".equals(cmd.text())),
            "最近的裂口应显示在 #1 行"
        );
        // 验证 200m 那个不在
        assertTrue(
            out.stream().noneMatch(cmd -> cmd.isText() && cmd.text().contains("200m")),
            "200m 远的裂口被裁剪掉，不应出现"
        );
    }

    @Test
    void collapseRiftListFiltersOtherFamilies() {
        // 非塌缩 family 的 collapse_tear 不应出现（防止跨副本数据泄漏）。
        java.util.List<RiftPortalView> portals = List.of(
            new RiftPortalView(1L, "collapse_tear", "exit", "other_family", 1, 64, 0, 2.0, 60, null),
            new RiftPortalView(2L, "collapse_tear", "exit", "tsy_lingxu_01", 5, 64, 0, 2.0, 60, null)
        );
        ExtractState state = new ExtractState(
            portals, null, "", 0, 0, false, "", 0xFFFFFFFF, 0L,
            "tsy_lingxu_01", 1000L, 60, 0L, 0, 1000L
        );

        java.util.List<HudRenderCommand> out = new java.util.ArrayList<>();
        ExtractProgressHudPlanner.appendCollapseRiftListWithPlayerPos(
            out, state, FIXED_WIDTH, 960, 540, new net.minecraft.util.math.Vec3d(0, 64, 0)
        );
        long lineCount = out.stream().filter(cmd -> cmd.isText() && cmd.text().startsWith("#")).count();
        org.junit.jupiter.api.Assertions.assertEquals(1L, lineCount, "应只列本族裂口");
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
