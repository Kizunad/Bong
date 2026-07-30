package com.bong.client.debug;

import com.bong.client.hud.AnqiHudState;
import com.bong.client.hud.AnqiHudStateStore;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandManager;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.text.Text;

public final class BongHudCommand {
    private static final String ROOT = "bonghud";
    private static final long DEFAULT_DURATION_MS = 1_500L;

    private BongHudCommand() {}

    public static void register() {
        ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) -> registerTo(dispatcher));
    }

    public static void registerTo(CommandDispatcher<FabricClientCommandSource> dispatcher) {
        dispatcher.register(ClientCommandManager.literal(ROOT)
            .then(ClientCommandManager.literal("aim_enclose")
                .then(ClientCommandManager.literal("mastery")
                    .then(ClientCommandManager.argument("mastery", IntegerArgumentType.integer(0, 100))
                        .executes(ctx -> setAim(ctx.getSource(), IntegerArgumentType.getInteger(ctx, "mastery") / 100f))))
                .then(ClientCommandManager.argument("progress", FloatArgumentType.floatArg(0f, 1f))
                    .executes(ctx -> setAim(ctx.getSource(), FloatArgumentType.getFloat(ctx, "progress")))))
            .then(ClientCommandManager.literal("charge_ring")
                .then(ClientCommandManager.argument("progress", FloatArgumentType.floatArg(0f, 1f))
                    .executes(ctx -> setCharge(ctx.getSource(), FloatArgumentType.getFloat(ctx, "progress")))))
            .then(ClientCommandManager.literal("echo_count")
                .then(ClientCommandManager.argument("n", IntegerArgumentType.integer(0))
                    .executes(ctx -> setEcho(ctx.getSource(), IntegerArgumentType.getInteger(ctx, "n")))))
            .then(ClientCommandManager.literal("abrasion_tooltip")
                .then(ClientCommandManager.argument("container", StringArgumentType.word())
                    .then(ClientCommandManager.argument("qi_payload", FloatArgumentType.floatArg(0f))
                        .executes(ctx -> setAbrasion(
                            ctx.getSource(),
                            StringArgumentType.getString(ctx, "container"),
                            FloatArgumentType.getFloat(ctx, "qi_payload"))))))
            .then(ClientCommandManager.literal("clear")
                .executes(ctx -> clear(ctx.getSource()))));
    }

    private static int setAim(FabricClientCommandSource source, float progress) {
        AnqiHudStateStore.replace(AnqiHudState.aim(progress, System.currentTimeMillis(), DEFAULT_DURATION_MS));
        source.sendFeedback(Text.literal("[bonghud] aim"));
        return 1;
    }

    private static int setCharge(FabricClientCommandSource source, float progress) {
        AnqiHudStateStore.replace(AnqiHudState.charge(progress, System.currentTimeMillis(), DEFAULT_DURATION_MS));
        source.sendFeedback(Text.literal("[bonghud] charge"));
        return 1;
    }

    private static int setEcho(FabricClientCommandSource source, int count) {
        AnqiHudStateStore.replace(AnqiHudState.echo(count, System.currentTimeMillis(), DEFAULT_DURATION_MS));
        source.sendFeedback(Text.literal("[bonghud] echo"));
        return 1;
    }

    private static int setAbrasion(FabricClientCommandSource source, String container, float qiPayload) {
        AnqiHudStateStore.replace(AnqiHudState.abrasion(container, qiPayload, System.currentTimeMillis(), DEFAULT_DURATION_MS));
        source.sendFeedback(Text.literal("[bonghud] abrasion"));
        return 1;
    }

    private static int clear(FabricClientCommandSource source) {
        AnqiHudStateStore.clear();
        source.sendFeedback(Text.literal("[bonghud] clear"));
        return 1;
    }
}
