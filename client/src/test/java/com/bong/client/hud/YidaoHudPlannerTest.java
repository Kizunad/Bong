package com.bong.client.hud;

import com.bong.client.yidao.YidaoHudStateStore;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertTrue;

class YidaoHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer WIDTH = text -> text.length() * 6;

    @Test
    void hidden_when_snapshot_is_empty() {
        assertTrue(YidaoHudPlanner.buildCommands(YidaoHudStateStore.Snapshot.EMPTY, WIDTH, 960, 540).isEmpty());
    }

    @Test
    void renders_healer_patient_karma_and_mass_preview() {
        YidaoHudStateStore.Snapshot state = new YidaoHudStateStore.Snapshot(
            "npc:doctor",
            7,
            48f,
            3.5,
            "life_extension",
            List.of("offline:Patient"),
            0.5f,
            1.25,
            1,
            2,
            4
        );

        List<HudRenderCommand> commands = YidaoHudPlanner.buildCommands(state, WIDTH, 960, 540);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains("医道 续命")));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains("信誉 7")));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains("HP 50%")));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains("群体 4")));
    }
}
