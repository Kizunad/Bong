package com.bong.client.debug;

import com.bong.client.animation.BongAnimationPlayer;
import com.bong.client.animation.BongAnimationRegistry;
import com.bong.client.animation.BongPunchCombo;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.suggestion.Suggestions;
import com.mojang.brigadier.suggestion.SuggestionsBuilder;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandManager;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.command.argument.IdentifierArgumentType;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

import java.util.Locale;
import java.util.concurrent.CompletableFuture;

/**
 * 开发期动画触发命令：
 * <ul>
 *   <li>{@code /anim list}：列出所有已注册动画 id</li>
 *   <li>{@code /anim test <id>}：在本地玩家身上播动画（默认优先级 1000，等同战斗动作档）</li>
 *   <li>{@code /anim test <id> <priority>}：自定义优先级，方便试验多层叠加</li>
 *   <li>{@code /anim test <id> <priority> <fade_in_ticks>}：自定义淡入 tick 数（0..40）</li>
 *   <li>{@code /anim stop <id>}：停掉指定动画</li>
 *   <li>{@code /anim combo punch}：触发右直拳组合拳（anim + 屏幕微震动 + 拳风声 + 粒子）</li>
 * </ul>
 *
 * <p>只在客户端运行，对 Valence 服务端无侵入。
 * 与 {@code /vfx} 保持同款参数风格（effect 名 tab 补全、参数可选）。
 *
 * <p><b>为什么 id 用 {@link IdentifierArgumentType} 而不是 {@code StringArgumentType.greedyString()}</b>：
 * greedyString 会把后面的内容（包括 priority / fade_in_ticks）全吞进 id 里，后续 int 参数永远到不了
 * ——直接破坏了 {@code <id> <priority>} 的命令契约。IdentifierArgumentType 按 {@code namespace:path}
 * 格式读到空白就停，天然适合链后续 int 参数；{@code word()/string()} 都不允许 {@code :} 字符，用不了。
 */
public final class BongAnimCommand {
    private static final String ROOT = "anim";
    /** plan-player-animation-v1 §3.3 的"战斗动作"基础档。 */
    public static final int DEFAULT_PRIORITY = 1000;

    private BongAnimCommand() {
    }

    public static void register() {
        ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) ->
            registerTo(dispatcher));
    }

    /**
     * 把命令树接到给定 dispatcher 上。拆出来单独暴露，方便单测用一个本地 dispatcher 验证参数树结构
     * ——无需触发 Fabric 的事件总线或启动真正的游戏实例。
     */
    static void registerTo(CommandDispatcher<FabricClientCommandSource> dispatcher) {
        dispatcher.register(ClientCommandManager.literal(ROOT)
            .then(ClientCommandManager.literal("list")
                .executes(BongAnimCommand::executeList))
            .then(ClientCommandManager.literal("test")
                .then(ClientCommandManager.argument("id", IdentifierArgumentType.identifier())
                    .suggests(BongAnimCommand::suggestAnimIds)
                    .executes(ctx -> executeTest(
                        ctx,
                        DEFAULT_PRIORITY,
                        BongAnimationPlayer.DEFAULT_FADE_IN_TICKS))
                    .then(ClientCommandManager.argument("priority", IntegerArgumentType.integer(0))
                        .executes(ctx -> executeTest(
                            ctx,
                            IntegerArgumentType.getInteger(ctx, "priority"),
                            BongAnimationPlayer.DEFAULT_FADE_IN_TICKS))
                        .then(ClientCommandManager.argument("fade_in_ticks", IntegerArgumentType.integer(0, 40))
                            .executes(ctx -> executeTest(
                                ctx,
                                IntegerArgumentType.getInteger(ctx, "priority"),
                                IntegerArgumentType.getInteger(ctx, "fade_in_ticks")))))))
            .then(ClientCommandManager.literal("stop")
                .then(ClientCommandManager.argument("id", IdentifierArgumentType.identifier())
                    .suggests(BongAnimCommand::suggestAnimIds)
                    .executes(BongAnimCommand::executeStop)))
            .then(ClientCommandManager.literal("combo")
                .then(ClientCommandManager.literal("punch")
                    .executes(BongAnimCommand::executeComboPunch)))
        );
    }

    private static int executeList(CommandContext<FabricClientCommandSource> ctx) {
        var ids = BongAnimationRegistry.ids();
        if (ids.isEmpty()) {
            ctx.getSource().sendFeedback(Text.literal("[bong/anim] 当前没有已注册的动画"));
            return 0;
        }
        // 每行一条，后面附来源标签 [JSON]/[JAVA] 方便确认 F3+T 重载后 JSON 是否生效
        StringBuilder sb = new StringBuilder("[bong/anim] 已注册 " + ids.size() + " 个:");
        for (Identifier id : ids) {
            sb.append("\n  ").append(id).append(" [").append(BongAnimationRegistry.sourceOf(id)).append(']');
        }
        ctx.getSource().sendFeedback(Text.literal(sb.toString()));
        return ids.size();
    }

    private static int executeTest(CommandContext<FabricClientCommandSource> ctx, int priority, int fadeInTicks) {
        Identifier id = ctx.getArgument("id", Identifier.class);

        ClientPlayerEntity player = MinecraftClient.getInstance().player;
        if (player == null) {
            ctx.getSource().sendError(Text.literal("[bong/anim] 本地玩家不存在（世界未加载？）"));
            return 0;
        }
        if (!BongAnimationRegistry.contains(id)) {
            ctx.getSource().sendError(Text.literal(
                "[bong/anim] 未注册的动画 id: " + id + "（/anim list 查看可用）"
            ));
            return 0;
        }

        boolean ok = BongAnimationPlayer.play(player, id, priority, fadeInTicks);
        if (!ok) {
            ctx.getSource().sendError(Text.literal("[bong/anim] 播放失败: " + id));
            return 0;
        }
        ctx.getSource().sendFeedback(Text.literal(String.format(Locale.ROOT,
            "[bong/anim] 播放 %s (priority=%d, fade_in=%d)", id, priority, fadeInTicks
        )));
        return 1;
    }

    private static int executeComboPunch(CommandContext<FabricClientCommandSource> ctx) {
        ClientPlayerEntity player = MinecraftClient.getInstance().player;
        if (player == null) {
            ctx.getSource().sendError(Text.literal("[bong/anim] 本地玩家不存在（世界未加载？）"));
            return 0;
        }
        boolean ok = BongPunchCombo.trigger(player);
        if (!ok) {
            ctx.getSource().sendError(Text.literal("[bong/anim] 组合拳触发失败"));
            return 0;
        }
        ctx.getSource().sendFeedback(Text.literal(
            "[bong/anim] 组合拳触发：fist_punch_right + 屏幕微震 + 拳风声 + 粒子"
        ));
        return 1;
    }

    private static int executeStop(CommandContext<FabricClientCommandSource> ctx) {
        Identifier id = ctx.getArgument("id", Identifier.class);

        ClientPlayerEntity player = MinecraftClient.getInstance().player;
        if (player == null) {
            ctx.getSource().sendError(Text.literal("[bong/anim] 本地玩家不存在"));
            return 0;
        }
        boolean ok = BongAnimationPlayer.stop(player, id);
        ctx.getSource().sendFeedback(Text.literal(
            "[bong/anim] 停止 " + id + (ok ? "（已淡出）" : "（未在播）")
        ));
        return ok ? 1 : 0;
    }

    /** 共享的 id tab 补全：直接把注册表里的 id.toString() 喂回去。 */
    private static CompletableFuture<Suggestions> suggestAnimIds(
        CommandContext<FabricClientCommandSource> ctx,
        SuggestionsBuilder builder
    ) {
        String remaining = builder.getRemaining().toLowerCase(Locale.ROOT);
        for (Identifier id : BongAnimationRegistry.ids()) {
            String s = id.toString();
            if (s.toLowerCase(Locale.ROOT).startsWith(remaining)) {
                builder.suggest(s);
            }
        }
        return builder.buildFuture();
    }
}
