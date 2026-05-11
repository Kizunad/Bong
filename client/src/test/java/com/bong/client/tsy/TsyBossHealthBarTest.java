package com.bong.client.tsy;

import com.bong.client.hud.HudRenderCommand;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TsyBossHealthBarTest {
    @AfterEach
    void reset() {
        TsyBossHealthStore.resetForTests();
        TsyDeathVfxStore.resetForTests();
    }

    @Test
    void boss_health_bar_phases() {
        TsyBossHealthState state = new TsyBossHealthState(true, "守灵", "通灵", 0.66, 2, 4, 1_000L);

        List<HudRenderCommand> commands = TsyBossHealthBar.buildCommands(state, 1_050L, text -> text.length() * 6, 320);

        assertFalse(commands.isEmpty());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains("守灵")));
        assertTrue(commands.stream().filter(HudRenderCommand::isRect).count() >= 6);
    }

    @Test
    void boss_bar_hidden_in_normal_combat() {
        List<HudRenderCommand> commands = TsyBossHealthBar.buildCommands(
            TsyBossHealthState.empty(),
            1_000L,
            text -> text.length() * 6,
            320
        );

        assertTrue(commands.isEmpty());
    }

    @Test
    void corpse_death_vfx_expires_after_one_second() {
        TsyDeathVfxState state = new TsyDeathVfxState(true, 1_000L);

        assertFalse(TsyCorpseDeathVfx.buildCommands(state, 1_500L, 320, 180).isEmpty());
        assertTrue(TsyCorpseDeathVfx.buildCommands(state, 2_100L, 320, 180).isEmpty());
    }
}
