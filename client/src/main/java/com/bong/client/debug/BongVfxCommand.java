package com.bong.client.debug;

import com.bong.client.BongClientFeatures;
import com.bong.client.hud.BongHudStateSnapshot;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.state.VisualEffectState;
import com.bong.client.visual.VisualEffectController;
import com.mojang.brigadier.arguments.DoubleArgumentType;
import com.mojang.brigadier.arguments.LongArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.suggestion.Suggestions;
import com.mojang.brigadier.suggestion.SuggestionsBuilder;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandManager;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.text.Text;

import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.CompletableFuture;
import java.util.stream.Collectors;

/**
 * Dev/QA 用视觉特效触发命令：<br>
 *   /vfx list<br>
 *   /vfx clear<br>
 *   /vfx &lt;effect&gt; [intensity 0..1] [duration_ms]<br>
 *
 * 所有 effect 名从 {@link VisualEffectState.EffectType} 自动派生，
 * 以后新增 EffectType 自动出现在 tab 补全与 list 输出里——不用改命令类。
 *
 * plan-vfx-v1 §8：本命令本身就是"整体集成 demo"的手动入口，
 * 跑 ./gradlew runClient 后在游戏内 `/vfx <effect>` 验证每档 HUD 叠色是否符合预期。
 */
public final class BongVfxCommand {
    /** 命令根节点 literal，所有测试命令都挂在 /vfx 命名空间下。 */
    private static final String ROOT = "vfx";
    /** 不传参时的默认强度（0..1）。 */
    private static final double DEFAULT_INTENSITY = 1.0;
    /** 不传参时的默认 duration。短于 profile.maxDurationMillis 时 controller 会按实际值存；
     *  profile 会封顶，所以此值仅作为"触发即见到完整时长"的合理上限。 */
    private static final long DEFAULT_DURATION_MS = 8_000L;

    private BongVfxCommand() {
    }

    public static void register() {
        ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) ->
            dispatcher.register(ClientCommandManager.literal(ROOT)
                .then(ClientCommandManager.literal("list")
                    .executes(BongVfxCommand::executeList))
                .then(ClientCommandManager.literal("clear")
                    .executes(BongVfxCommand::executeClear))
                .then(ClientCommandManager.argument("effect", StringArgumentType.word())
                    .suggests(BongVfxCommand::suggestEffects)
                    .executes(ctx -> executeTrigger(ctx, DEFAULT_INTENSITY, DEFAULT_DURATION_MS))
                    .then(ClientCommandManager.argument("intensity", DoubleArgumentType.doubleArg(0.0, 1.0))
                        .executes(ctx -> executeTrigger(
                            ctx,
                            DoubleArgumentType.getDouble(ctx, "intensity"),
                            DEFAULT_DURATION_MS))
                        .then(ClientCommandManager.argument("duration_ms", LongArgumentType.longArg(1L))
                            .executes(ctx -> executeTrigger(
                                ctx,
                                DoubleArgumentType.getDouble(ctx, "intensity"),
                                LongArgumentType.getLong(ctx, "duration_ms"))))))
            )
        );
    }

    /** 暴露给测试：动态返回所有非 NONE 的 EffectType 名称。 */
    public static List<String> availableEffectNames() {
        return Arrays.stream(VisualEffectState.EffectType.values())
            .filter(t -> t != VisualEffectState.EffectType.NONE)
            .map(VisualEffectState.EffectType::wireName)
            .collect(Collectors.toUnmodifiableList());
    }

    private static int executeList(CommandContext<FabricClientCommandSource> ctx) {
        String joined = String.join(", ", availableEffectNames());
        ctx.getSource().sendFeedback(Text.literal("[bong/vfx] 已注册效果: " + joined));
        return availableEffectNames().size();
    }

    private static int executeClear(CommandContext<FabricClientCommandSource> ctx) {
        BongHudStateSnapshot current = BongHudStateStore.snapshot();
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            current.zoneState(),
            current.narrationState(),
            VisualEffectState.none()
        ));
        ctx.getSource().sendFeedback(Text.literal("[bong/vfx] 已清除当前视觉效果"));
        return 1;
    }

    private static int executeTrigger(
        CommandContext<FabricClientCommandSource> ctx,
        double intensity,
        long durationMillis
    ) {
        String effectName = StringArgumentType.getString(ctx, "effect").trim().toLowerCase(Locale.ROOT);
        long nowMillis = System.currentTimeMillis();
        VisualEffectState incoming = VisualEffectState.create(effectName, intensity, durationMillis, nowMillis);
        if (incoming.isEmpty()) {
            ctx.getSource().sendError(Text.literal(
                "[bong/vfx] 未知效果或参数无效: " + effectName + "（使用 /vfx list 查看可用效果）"
            ));
            return 0;
        }

        BongHudStateSnapshot current = BongHudStateStore.snapshot();
        VisualEffectState previous = current.visualEffectState();
        VisualEffectState next = VisualEffectController.acceptIncoming(
            previous,
            incoming,
            nowMillis,
            BongClientFeatures.ENABLE_VISUAL_EFFECTS
        );
        BongHudStateStore.replace(BongHudStateSnapshot.create(
            current.zoneState(),
            current.narrationState(),
            next
        ));

        if (!BongClientFeatures.ENABLE_VISUAL_EFFECTS) {
            ctx.getSource().sendError(Text.literal("[bong/vfx] ENABLE_VISUAL_EFFECTS 关闭，触发无效"));
            return 0;
        }

        // 被 retrigger 窗口拦截时 next 与 previous 的 started_at 一致
        boolean sameLifetime = next.effectType() == previous.effectType()
            && next.startedAtMillis() == previous.startedAtMillis()
            && !previous.isEmpty();
        if (sameLifetime) {
            ctx.getSource().sendFeedback(Text.literal(
                "[bong/vfx] 触发被重触发窗口拦截: " + effectName + "（先 /vfx clear 再试）"
            ));
            return 0;
        }

        ctx.getSource().sendFeedback(Text.literal(String.format(Locale.ROOT,
            "[bong/vfx] 触发 %s (intensity=%.2f, duration=%dms) → 实际 %s intensity=%.2f duration=%dms",
            effectName, intensity, durationMillis,
            next.effectType().wireName(), next.intensity(), next.durationMillis()
        )));
        return 1;
    }

    private static CompletableFuture<Suggestions> suggestEffects(
        CommandContext<FabricClientCommandSource> ctx,
        SuggestionsBuilder builder
    ) {
        String remaining = builder.getRemaining().toLowerCase(Locale.ROOT);
        for (String name : availableEffectNames()) {
            if (name.startsWith(remaining)) {
                builder.suggest(name);
            }
        }
        return builder.buildFuture();
    }
}
