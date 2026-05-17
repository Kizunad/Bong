package com.bong.client.iris;

import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.suggestion.Suggestions;
import com.mojang.brigadier.suggestion.SuggestionsBuilder;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandManager;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.text.Text;

import java.util.Locale;
import java.util.concurrent.CompletableFuture;

public final class BongShaderCommand {
    private static final String ROOT = "bong_shader";

    private BongShaderCommand() {
    }

    public static void register() {
        ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) ->
                dispatcher.register(ClientCommandManager.literal(ROOT)
                        .then(ClientCommandManager.literal("list")
                                .executes(BongShaderCommand::execList))
                        .then(ClientCommandManager.literal("set")
                                .then(ClientCommandManager.argument("name", StringArgumentType.word())
                                        .suggests(BongShaderCommand::suggestUniforms)
                                        .then(ClientCommandManager.argument("value", FloatArgumentType.floatArg(0f, 6.2832f))
                                                .executes(BongShaderCommand::execSet))))
                        .then(ClientCommandManager.literal("reset")
                                .executes(BongShaderCommand::execReset))
                        .then(ClientCommandManager.literal("dump")
                                .executes(BongShaderCommand::execDump))
                )
        );
    }

    private static int execList(CommandContext<FabricClientCommandSource> ctx) {
        StringBuilder sb = new StringBuilder("[BongShader] Uniforms:\n");
        for (BongUniform u : BongUniform.values()) {
            String override = BongShaderState.isOverridden(u) ? " [OVERRIDE]" : "";
            sb.append(String.format(Locale.ROOT, "  %s = %.4f (target=%.4f)%s\n",
                    u.shaderName(), BongShaderState.get(u), BongShaderState.getTarget(u), override));
        }
        ctx.getSource().sendFeedback(Text.literal(sb.toString()));
        return 1;
    }

    private static int execSet(CommandContext<FabricClientCommandSource> ctx) {
        String name = StringArgumentType.getString(ctx, "name");
        float value = FloatArgumentType.getFloat(ctx, "value");
        BongUniform uniform = BongUniform.fromShaderName(name);
        if (uniform == null) {
            ctx.getSource().sendError(Text.literal("[BongShader] Unknown uniform: " + name));
            return 0;
        }
        BongShaderState.setOverride(uniform, value);
        float applied = BongShaderState.get(uniform);
        ctx.getSource().sendFeedback(Text.literal(
                String.format(Locale.ROOT, "[BongShader] Override %s = %.4f", uniform.shaderName(), applied)));
        return 1;
    }

    private static int execReset(CommandContext<FabricClientCommandSource> ctx) {
        BongShaderState.clearAllOverrides();
        ctx.getSource().sendFeedback(Text.literal("[BongShader] All overrides cleared, server-driven mode restored."));
        return 1;
    }

    private static int execDump(CommandContext<FabricClientCommandSource> ctx) {
        StringBuilder sb = new StringBuilder("[BongShader] Dump:\n");
        sb.append("  Iris available: ").append(BongIrisCompat.isAvailable()).append("\n");
        sb.append("  Iris version: ").append(BongIrisCompat.getIrisVersion()).append("\n");
        sb.append("  Uniform count: ").append(BongUniform.values().length).append("\n");
        int overrideCount = 0;
        for (BongUniform u : BongUniform.values()) {
            if (BongShaderState.isOverridden(u)) {
                overrideCount++;
            }
        }
        sb.append("  Overridden: ").append(overrideCount);
        ctx.getSource().sendFeedback(Text.literal(sb.toString()));
        return 1;
    }

    private static CompletableFuture<Suggestions> suggestUniforms(
            CommandContext<FabricClientCommandSource> ctx, SuggestionsBuilder builder) {
        String remaining = builder.getRemaining().toLowerCase(Locale.ROOT);
        for (BongUniform u : BongUniform.values()) {
            if (u.shaderName().startsWith(remaining)) {
                builder.suggest(u.shaderName());
            }
        }
        return builder.buildFuture();
    }
}
