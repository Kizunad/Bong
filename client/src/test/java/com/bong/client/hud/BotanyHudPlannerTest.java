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

        List<HudRenderCommand> commands = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 320, 240);

        assertFalse(commands.isEmpty());
        assertEquals(HudRenderLayer.BOTANY, commands.get(0).layer());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("采集 · 开脉草")));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("60%")));
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

        List<HudRenderCommand> commands = BotanyHudPlanner.buildCommands(session, skill, FIXED_WIDTH, 320, 240);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && cmd.text().contains("需采药 Lv.3")));
    }
}
