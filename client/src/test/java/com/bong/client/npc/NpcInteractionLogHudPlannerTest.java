package com.bong.client.npc;

import com.bong.client.hud.HudRenderCommand;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NpcInteractionLogHudPlannerTest {
    @AfterEach
    void reset() {
        NpcInteractionLogStore.resetForTests();
    }

    @Test
    void interaction_log_max_10() {
        for (int i = 0; i < 12; i++) {
            NpcInteractionLogStore.record(new NpcInteractionLogEntry(i, "NPC " + i, "rogue", "greeting", i));
        }

        assertEquals(10, NpcInteractionLogStore.snapshot().size());
        assertEquals(11, NpcInteractionLogStore.snapshot().get(0).entityId());
    }

    @Test
    void hidden_log_emits_no_commands() {
        List<HudRenderCommand> commands = NpcInteractionLogHudPlanner.buildCommands(
            List.of(new NpcInteractionLogEntry(1, "散修", "rogue", "trade", 1_000L)),
            false,
            text -> text.length() * 6,
            320,
            180
        );

        assertTrue(commands.isEmpty());
    }
}
