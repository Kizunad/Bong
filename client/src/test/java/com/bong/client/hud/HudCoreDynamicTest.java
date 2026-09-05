package com.bong.client.hud;

import com.bong.client.combat.CastState;
import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/** 动态投影契约：只缓存资源几何，不能缓存某一帧的数值、槽位或时间。 */
class HudCoreDynamicTest {
    @Test
    void cooldownShrinksAndExpiresForBothRowsWithoutMutatingSnapshots() {
        QuickSlotConfig quick = QuickSlotConfig.of(
            new QuickSlotEntry[] {new QuickSlotEntry("herb", "药草", 0, 2_000, "")}, new long[] {3_000});
        SkillBarConfig skills = SkillBarConfig.of(
            new SkillBarEntry[] {SkillBarEntry.skill("parry", "弹反", 0, 2_000, "")}, new long[] {3_000});
        List<HudRenderCommand> full = cooldowns(quick, skills, 1_000);
        List<HudRenderCommand> half = cooldowns(quick, skills, 2_000);
        assertEquals(2, full.size(), "两排都需显示各自的冷却");
        assertEquals(full.size(), half.size());
        for (int i = 0; i < full.size(); i++) {
            assertEquals(full.get(i).height() / 2, half.get(i).height(), "过半时间遮罩应降到半高");
            assertEquals(full.get(i).y() + full.get(i).height(), half.get(i).y() + half.get(i).height(),
                "遮罩底部不能在倒计时过程中移动");
        }
        assertTrue(cooldowns(quick, skills, 3_000).isEmpty(), "截止时刻不得残留冷却遮罩");
        assertEquals(3_000, quick.cooldownUntilMs(0));
        assertEquals(3_000, skills.cooldownUntilMs(0));
    }

    private static List<HudRenderCommand> cooldowns(QuickSlotConfig quick, SkillBarConfig skills, long now) {
        return QuickBarHudPlanner.buildCommands(quick, skills, 0, CastState.idle(), List.of(), now, 320, 240).stream()
            .filter(c -> c.isVector() && c.color() == QuickBarHudPlanner.COOLDOWN_OVERLAY_COLOR).toList();
    }

    @Test
    void selectedSlotAndCastProgressAreRecomputedPerFrame() {
        SkillBarConfig skills = SkillBarConfig.of(
            new SkillBarEntry[] {SkillBarEntry.skill("parry", "弹反", 4_000, 0, "")}, null);
        CastState cast = CastState.casting(CastState.Source.SKILL_BAR, 0, 4_000, 1_000);
        List<HudRenderCommand> early = QuickBarHudPlanner.buildCommands(
            null, skills, 0, cast, List.of(), 2_000, 320, 240);
        List<HudRenderCommand> later = QuickBarHudPlanner.buildCommands(
            null, skills, 2, cast, List.of(), 4_000, 320, 240);
        HudRenderCommand selectedEarly = colored(early, QuickBarHudPlanner.SELECTED_BORDER_COLOR);
        HudRenderCommand selectedLater = colored(later, QuickBarHudPlanner.SELECTED_BORDER_COLOR);
        assertEquals(2 * (QuickBarHudPlanner.SLOT_SIZE + QuickBarHudPlanner.SLOT_GAP),
            selectedLater.x() - selectedEarly.x(), "选中框应移到新的槽位");
        assertEquals(QuickBarHudPlanner.SLOT_SIZE / 4, colored(early, QuickBarHudPlanner.CAST_BAR_FG).width());
        assertEquals(QuickBarHudPlanner.SLOT_SIZE * 3 / 4, colored(later, QuickBarHudPlanner.CAST_BAR_FG).width());
        assertTrue(QuickBarHudPlanner.buildCommands(null, skills, 2, CastState.idle(), List.of(), 5_000, 320, 240)
            .stream().noneMatch(c -> c.layer() == HudRenderLayer.CAST_BAR), "施法退出后必须清掉进度条");
    }

    @Test
    void miniBodyAvoidsBothQuickRowsAndReservedWeaponSlotsAtSupportedViewports() {
        for (int[] viewport : new int[][] {{320, 240}, {401, 241}, {683, 384}, {320, 480}, {1280, 360}}) {
            int width = viewport[0], height = viewport[1];
            var mini = MiniBodyHudPlanner.buildCommands(
                CombatHudState.create(1, 1, 1, DerivedAttrFlags.none()), null, null, 500, width, height);
            HudRenderCommand panel = colored(mini, MiniBodyHudPlanner.PANEL_BG_COLOR);
            var quick = QuickBarHudPlanner.buildCommands(null, 0, CastState.idle(), 500, width, height);
            int left = quick.stream().mapToInt(HudRenderCommand::x).min().orElseThrow()
                - WeaponHotbarHudPlanner.SLOT_GAP_TO_HOTBAR - WeaponHotbarHudPlanner.SLOT_W;
            int top = quick.stream().mapToInt(HudRenderCommand::y).min().orElseThrow();
            assertTrue(panel.x() + panel.width() <= left || panel.y() + panel.height() <= top,
                "人体不得遮挡快捷栏及武器槽: " + width + "x" + height);
            for (HudRenderCommand command : mini) {
                assertTrue(command.x() >= 0 && command.y() >= 0
                    && command.x() + command.width() <= width && command.y() + command.height() <= height,
                    "人体图形不得出界: " + width + "x" + height);
            }
        }
    }

    @Test
    void vectorFadeKeepsAssetAndBounds() {
        HudRenderCommand source = HudRenderCommand.vector(HudRenderLayer.MINI_BODY, "body", 6, 8, 30, 75, 0xCC112233);
        HudRenderCommand faded = HudCommandAlpha.withAlpha(source, 0.5);
        assertTrue(faded.isVector());
        assertEquals(source.text(), faded.text());
        assertEquals(source.layer(), faded.layer());
        assertEquals(source.x(), faded.x());
        assertEquals(source.y(), faded.y());
        assertEquals(source.width(), faded.width());
        assertEquals(source.height(), faded.height());
        assertEquals(0x66112233, faded.color(), "沉浸模式只能衰减透明度，不得改变色相或几何");
    }

    private static HudRenderCommand colored(List<HudRenderCommand> commands, int color) {
        return commands.stream().filter(c -> c.color() == color).findFirst().orElseThrow();
    }
}
