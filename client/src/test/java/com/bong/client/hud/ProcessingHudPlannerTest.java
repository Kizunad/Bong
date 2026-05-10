package com.bong.client.hud;

import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.forge.state.ForgeSessionStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import net.minecraft.util.math.BlockPos;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ProcessingHudPlannerTest {
    @AfterEach
    void reset() {
        ForgeSessionStore.resetForTests();
        ForgeProgressHudPlanner.resetForTests();
        AlchemySessionStore.resetForTests();
        AlchemyFurnaceStore.resetForTests();
    }

    @Test
    void forgeStepProgressShowsLabelAndBar() {
        ForgeSessionStore.replace(new ForgeSessionStore.Snapshot(
            7L,
            "iron_sword",
            "铁剑",
            true,
            "inscription",
            1,
            2,
            "{\"progress\":0.5}"
        ));

        List<HudRenderCommand> commands = ForgeProgressHudPlanner.buildCommands(320, 180, 2_000L);

        assertEquals(0.5, ForgeProgressHudPlanner.progressOf(ForgeSessionStore.snapshot()), 1e-6);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.text().contains("铭文刻划")));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.PROCESSING_HUD && cmd.isRect()));
    }

    @Test
    void alchemyTemperatureBarUsesHeatColor() {
        AlchemyFurnaceStore.replace(new AlchemyFurnaceStore.Snapshot(new BlockPos(0, 64, 0), 1, 92f, 100f, "self", true));
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot(
            "kaimai_pill",
            50,
            100,
            0.9f,
            0.5f,
            0.1f,
            5.0,
            10.0,
            "过热",
            List.of(),
            List.of()
        ));

        List<HudRenderCommand> commands = AlchemyProgressHudPlanner.buildCommands(320, 180);

        assertEquals(0.5, AlchemyProgressHudPlanner.progressOf(AlchemySessionStore.snapshot()), 1e-6);
        assertTrue(commands.stream().anyMatch(cmd -> cmd.color() == 0xFFE06040));
    }
}
