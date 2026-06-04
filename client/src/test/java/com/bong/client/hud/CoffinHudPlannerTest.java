package com.bong.client.hud;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CoffinHudPlannerTest {
    @Test
    void hidesWhenPlayerIsOutOfCoffin() {
        assertTrue(CoffinHudPlanner.buildCommands(CoffinStateStore.OUT, 800, 600).isEmpty());
    }

    @Test
    void showsLabelAndMultiplierWhenPlayerIsInCoffin() {
        List<HudRenderCommand> commands = CoffinHudPlanner.buildCommands(
            new CoffinStateStore.State(true, 0.9, null),
            800,
            600
        );

        // P3 新增档位徽章 → 4 个命令（rect + LABEL + badge + multiplier）。
        assertEquals(4, commands.size());
        assertEquals(HudRenderLayer.COFFIN, commands.get(0).layer());
        assertTrue(commands.stream().anyMatch(cmd -> CoffinHudPlanner.LABEL.equals(cmd.text())));
        assertTrue(commands.stream().anyMatch(cmd -> "×0.9".equals(cmd.text())));
    }

    // ─── plan-coffin-tiers-v1 P3：档位徽章 ──────────────────────────────────

    @ParameterizedTest(name = "grade={0} → badge={1}")
    @CsvSource({
        "mundane, 凡木",
        "jade,    寒玉",
        "stone,   玄石",
        "bronze,  青铜"
    })
    void gradeBadgeReturnsCorrectHanzi(String grade, String expectedHanzi) {
        assertEquals(expectedHanzi, CoffinHudPlanner.gradeBadge(grade),
            "grade='" + grade + "' should produce badge '" + expectedHanzi + "'");
    }

    @Test
    void gradeBadgeNullDefaultsMundane() {
        assertEquals("凡木", CoffinHudPlanner.gradeBadge(null),
            "null grade should default to '凡木' (Mundane)");
    }

    @Test
    void gradeBadgeUnknownDefaultsMundane() {
        assertEquals("凡木", CoffinHudPlanner.gradeBadge("diamond"),
            "unknown grade should default to '凡木' (forward-compat defense)");
    }

    @ParameterizedTest(name = "grade={0} multiplier={1} badge={2}")
    @CsvSource({
        "mundane, 0.9, 凡木, ×0.9",
        "jade,    0.7, 寒玉, ×0.7",
        "stone,   0.5, 玄石, ×0.5",
        "bronze,  0.3, 青铜, ×0.3"
    })
    void hudPanelShowsBadgeAndMultiplierForAllGrades(
        String grade, double multiplier, String badgeHanzi, String multiplierText
    ) {
        List<HudRenderCommand> commands = CoffinHudPlanner.buildCommands(
            new CoffinStateStore.State(true, multiplier, grade),
            800, 600
        );

        // 4 commands：rect + LABEL text + badge text + multiplier text
        assertEquals(4, commands.size(),
            "grade=" + grade + " should produce 4 HUD commands (rect+label+badge+multiplier)");
        assertTrue(
            commands.stream().anyMatch(cmd -> ("[" + badgeHanzi + "]").equals(cmd.text())),
            "grade=" + grade + " should show badge '[" + badgeHanzi + "]' in HUD commands"
        );
        assertTrue(
            commands.stream().anyMatch(cmd -> multiplierText.equals(cmd.text())),
            "grade=" + grade + " should show '" + multiplierText + "' in HUD commands"
        );
        assertTrue(
            commands.stream().anyMatch(cmd -> CoffinHudPlanner.LABEL.equals(cmd.text())),
            "LABEL should always appear in coffin HUD"
        );
    }

    @Test
    void hudPanelAllCommandsUseCoffinLayer() {
        List<HudRenderCommand> commands = CoffinHudPlanner.buildCommands(
            new CoffinStateStore.State(true, 0.7, "jade"),
            800, 600
        );
        assertTrue(
            commands.stream().allMatch(cmd -> HudRenderLayer.COFFIN == cmd.layer()),
            "all coffin HUD commands should use HudRenderLayer.COFFIN"
        );
    }
}
