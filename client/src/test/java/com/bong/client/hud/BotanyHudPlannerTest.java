package com.bong.client.hud;

import com.bong.client.botany.BotanyHarvestMode;
import com.bong.client.botany.BotanySkillViewModel;
import com.bong.client.botany.HarvestSessionViewModel;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @Test
    void singleActiveSessionBuildsPopupCommands() {
        HarvestSessionViewModel session = HarvestSessionViewModel.create(
            "session-1",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            BotanyHarvestMode.MANUAL,
            0.6,
            true,
            false,
            false,
            false,
            "晨露未散",
            10L
        );
        BotanySkillViewModel skill = BotanySkillViewModel.create(2, 40, 100, 3);

        List<HudRenderCommand> commands = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 960, 540);

        assertFalse(commands.isEmpty());
        assertEquals(HudRenderLayer.BOTANY, commands.get(0).layer());
        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("采集 · 开脉草")),
            "header should read `采集 · 开脉草`"
        );
        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("60%")),
            "progress label should show 60%"
        );
        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("ESC 取消")),
            "header hint should show ESC cancel prompt"
        );
        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("WASD")),
            "footer should list interrupt triggers"
        );
        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("5%")),
            "trample warning should be present"
        );
    }

    @Test
    void autoModeShowsLockedHintBelowUnlockThreshold() {
        HarvestSessionViewModel session = HarvestSessionViewModel.create(
            "session-1",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            null,
            0.0,
            true,
            false,
            false,
            false,
            "晨露未散",
            10L
        );
        BotanySkillViewModel skill = BotanySkillViewModel.create(1, 10, 100, 3);

        List<HudRenderCommand> commands = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 960, 540);

        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("需采药 Lv.3")),
            "locked auto button should show unlock threshold"
        );
    }

    @Test
    void projectionAnchorShiftsPanelTowardsPlant() {
        HarvestSessionViewModel session = HarvestSessionViewModel.create(
            "session-1",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            BotanyHarvestMode.MANUAL,
            0.4,
            true,
            false,
            false,
            false,
            "晨露未散",
            10L
        );
        BotanySkillViewModel skill = BotanySkillViewModel.create(2, 40, 100, 3);

        // 植物在屏幕左上方：panel 应向左上偏移
        BotanyProjection.Anchor anchor = new BotanyProjection.Anchor(100, 120, true);
        List<HudRenderCommand> anchored = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 960, 540, anchor);

        BotanyProjection.Anchor fallback = null;
        List<HudRenderCommand> defaultPos = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 960, 540, fallback);

        // 第一条 command 是阴影块，x 坐标代表 panel 左上
        int anchoredX = anchored.get(0).x();
        int defaultX = defaultPos.get(0).x();
        assertTrue(
            anchoredX < defaultX,
            "anchored panel should sit left of default (plant is at screen left), anchored=" + anchoredX + " default=" + defaultX
        );
    }

    @Test
    void offscreenAnchorFallsBackToDefaultPosition() {
        HarvestSessionViewModel session = HarvestSessionViewModel.create(
            "session-1",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            BotanyHarvestMode.MANUAL,
            0.4,
            true,
            false,
            false,
            false,
            "晨露未散",
            10L
        );
        BotanySkillViewModel skill = BotanySkillViewModel.create(2, 40, 100, 3);

        BotanyProjection.Anchor behind = new BotanyProjection.Anchor(0, 0, false);
        List<HudRenderCommand> behindList = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 960, 540, behind);
        List<HudRenderCommand> defaultList = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 960, 540, null);

        assertEquals(behindList.get(0).x(), defaultList.get(0).x());
        assertEquals(behindList.get(0).y(), defaultList.get(0).y());
    }

    @Test
    void knownPlantKindEmitsTexturedThumbnail() {
        HarvestSessionViewModel session = HarvestSessionViewModel.create(
            "session-1",
            "plant-1",
            "凝脉草",
            "ning_mai_cao",
            BotanyHarvestMode.MANUAL,
            0.3,
            true,
            false,
            false,
            false,
            "",
            10L
        );
        BotanySkillViewModel skill = BotanySkillViewModel.create(2, 40, 100, 3);

        List<HudRenderCommand> commands = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 960, 540);

        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isTexturedRect()
                && cmd.texturePath().contains("ning_mai_cao")),
            "known plant kind should emit TEXTURED_RECT with its icon path"
        );
    }

    @Test
    void unknownPlantKindFallsBackToColorBlockThumbnail() {
        HarvestSessionViewModel session = HarvestSessionViewModel.create(
            "session-1",
            "plant-X",
            "未知灵草",
            "some_unknown_kind",
            BotanyHarvestMode.MANUAL,
            0.3,
            true,
            false,
            false,
            false,
            "",
            10L
        );
        BotanySkillViewModel skill = BotanySkillViewModel.create(2, 40, 100, 3);

        List<HudRenderCommand> commands = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 960, 540);

        assertFalse(
            commands.stream().anyMatch(cmd -> cmd.isTexturedRect()),
            "unknown plant kind should NOT emit TEXTURED_RECT"
        );
    }

    @Test
    void interruptedSessionShowsDangerHeader() {
        HarvestSessionViewModel session = HarvestSessionViewModel.create(
            "session-1",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            BotanyHarvestMode.MANUAL,
            0.4,
            true,
            false,
            true,
            false,
            "移动打断",
            10L
        );
        BotanySkillViewModel skill = BotanySkillViewModel.create(2, 40, 100, 3);

        List<HudRenderCommand> commands = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 960, 540);

        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("已打断")),
            "interrupted session should show `已打断` header hint"
        );
        assertTrue(
            commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("中止")),
            "progress label should read `中止`"
        );
    }
}
