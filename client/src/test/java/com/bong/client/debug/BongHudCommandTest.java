package com.bong.client.debug;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.ParseResults;
import com.mojang.brigadier.context.CommandContext;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BongHudCommandTest {
    private CommandDispatcher<FabricClientCommandSource> buildDispatcher() {
        CommandDispatcher<FabricClientCommandSource> dispatcher = new CommandDispatcher<>();
        BongHudCommand.registerTo(dispatcher);
        return dispatcher;
    }

    private CommandContext<FabricClientCommandSource> parseArgs(String input) {
        CommandDispatcher<FabricClientCommandSource> dispatcher = buildDispatcher();
        ParseResults<FabricClientCommandSource> parse = dispatcher.parse(input, null);
        assertTrue(parse.getExceptions().isEmpty(),
            "expected clean parse but got: " + parse.getExceptions());
        CommandContext<FabricClientCommandSource> ctx = parse.getContext().build(input);
        assertNotNull(ctx.getCommand(), "parser should resolve a terminal executor for: " + input);
        return ctx;
    }

    @Test
    void parsesAimProgress() {
        CommandContext<FabricClientCommandSource> ctx = parseArgs("bonghud aim_enclose 1.0");

        assertEquals(1.0f, ctx.getArgument("progress", Float.class));
    }

    @Test
    void parsesAimMastery() {
        CommandContext<FabricClientCommandSource> ctx = parseArgs("bonghud aim_enclose mastery 100");

        assertEquals(100, ctx.getArgument("mastery", Integer.class));
    }

    @Test
    void parsesChargeRing() {
        CommandContext<FabricClientCommandSource> ctx = parseArgs("bonghud charge_ring 0.75");

        assertEquals(0.75f, ctx.getArgument("progress", Float.class));
    }

    @Test
    void parsesEchoCount() {
        CommandContext<FabricClientCommandSource> ctx = parseArgs("bonghud echo_count 3");

        assertEquals(3, ctx.getArgument("n", Integer.class));
    }

    @Test
    void parsesAbrasionTooltip() {
        CommandContext<FabricClientCommandSource> ctx = parseArgs("bonghud abrasion_tooltip jade_tube 12.5");

        assertEquals("jade_tube", ctx.getArgument("container", String.class));
        assertEquals(12.5f, ctx.getArgument("qi_payload", Float.class));
    }

    @Test
    void parsesClear() {
        parseArgs("bonghud clear");
    }
}
