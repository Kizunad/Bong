package com.bong.client.debug;

import com.bong.client.network.VfxEventEnvelope;
import com.bong.client.network.VfxEventPayload;
import com.bong.client.visual.particle.BongParticles;
import com.bong.client.visual.particle.BongVfxParticleBridge;
import com.bong.client.visual.particle.VfxRegistry;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.DoubleArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.suggestion.Suggestions;
import com.mojang.brigadier.suggestion.SuggestionsBuilder;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandManager;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.texture.Sprite;
import net.minecraft.command.argument.IdentifierArgumentType;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Vec3d;

import java.util.List;
import java.util.Locale;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.concurrent.CompletableFuture;

/**
 * 客户端 dev 粒子触发：
 * <ul>
 *   <li>{@code /spawnp list}：列出所有已注册 eventId</li>
 *   <li>{@code /spawnp debug}：扫 sprite provider 状态（紫黑格诊断）</li>
 *   <li>{@code /spawnp <event_id>}：在玩家正前方 {@value #DEFAULT_DISTANCE} 格处触发</li>
 *   <li>{@code /spawnp <event_id> <strength>}：自定义强度 (0..1)</li>
 *   <li>{@code /spawnp <event_id> <strength> <count>}：自定义粒子数 (1..{@link VfxEventEnvelope#VFX_PARTICLE_COUNT_MAX})</li>
 * </ul>
 *
 * <p>绕过 server，直接本地构造 {@link VfxEventPayload.SpawnParticle} 喂给 {@link BongVfxParticleBridge}。
 */
public final class BongSpawnParticleCommand {
    private static final String ROOT = "spawnp";
    private static final double DEFAULT_DISTANCE = 3.0;
    private static final double DEFAULT_STRENGTH = 0.8;
    private static final int DEFAULT_COUNT = 4;
    private static final int DEFAULT_DURATION_TICKS = 20;

    private BongSpawnParticleCommand() {
    }

    public static void register() {
        ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) ->
            registerTo(dispatcher));
    }

    static void registerTo(CommandDispatcher<FabricClientCommandSource> dispatcher) {
        dispatcher.register(ClientCommandManager.literal(ROOT)
            .then(ClientCommandManager.literal("list")
                .executes(BongSpawnParticleCommand::executeList))
            .then(ClientCommandManager.literal("debug")
                .executes(BongSpawnParticleCommand::executeDebug))
            .then(ClientCommandManager.argument("event_id", IdentifierArgumentType.identifier())
                .suggests(BongSpawnParticleCommand::suggestEventIds)
                .executes(ctx -> executeSpawn(ctx, DEFAULT_STRENGTH, DEFAULT_COUNT))
                .then(ClientCommandManager.argument("strength", DoubleArgumentType.doubleArg(0.0, 1.0))
                    .executes(ctx -> executeSpawn(
                        ctx,
                        DoubleArgumentType.getDouble(ctx, "strength"),
                        DEFAULT_COUNT))
                    .then(ClientCommandManager.argument("count", IntegerArgumentType.integer(1, VfxEventEnvelope.VFX_PARTICLE_COUNT_MAX))
                        .executes(ctx -> executeSpawn(
                            ctx,
                            DoubleArgumentType.getDouble(ctx, "strength"),
                            IntegerArgumentType.getInteger(ctx, "count"))))))
        );
    }

    public static List<String> availableEventIds() {
        return VfxRegistry.instance().ids().stream()
            .map(Identifier::toString)
            .sorted()
            .toList();
    }

    private static int executeList(CommandContext<FabricClientCommandSource> ctx) {
        List<String> ids = availableEventIds();
        if (ids.isEmpty()) {
            ctx.getSource().sendFeedback(Text.literal("[bong/spawnp] 当前没有已注册的粒子事件"));
            return 0;
        }
        ctx.getSource().sendFeedback(Text.literal(
            "[bong/spawnp] 已注册 " + ids.size() + " 个:\n  " + String.join("\n  ", ids)
        ));
        return ids.size();
    }

    private static int executeDebug(CommandContext<FabricClientCommandSource> ctx) {
        MinecraftClient client = MinecraftClient.getInstance();
        StringBuilder sb = new StringBuilder("[bong/spawnp debug] sprite provider 状态:");
        sb.append(reportSprite("sword_qi_trail",      BongParticles.swordQiTrailSprites,      client));
        sb.append(reportSprite("sword_slash_arc",     BongParticles.swordSlashArcSprites,     client));
        sb.append(reportSprite("breakthrough_pillar", BongParticles.breakthroughPillarSprites, client));
        sb.append(reportSprite("tribulation_spark",   BongParticles.tribulationSparkSprites,  client));
        sb.append(reportSprite("flying_sword_trail",  BongParticles.flyingSwordTrailSprites,  client));
        sb.append(reportSprite("lingqi_ripple",       BongParticles.lingqiRippleSprites,      client));
        sb.append(reportSprite("qi_aura",             BongParticles.qiAuraSprites,            client));
        sb.append(reportSprite("rune_char",           BongParticles.runeCharSprites,          client));
        sb.append(reportSprite("enlightenment_dust",  BongParticles.enlightenmentDustSprites, client));
        ctx.getSource().sendFeedback(Text.literal(sb.toString()));
        return 1;
    }

    private static String reportSprite(String name, SpriteProvider provider, MinecraftClient client) {
        if (provider == null) {
            return "\n  " + name + ": PROVIDER_NULL";
        }
        if (client.world == null) {
            return "\n  " + name + ": provider ok, 但 world 未加载";
        }
        try {
            Sprite sprite = provider.getSprite(client.world.random);
            if (sprite == null) {
                return "\n  " + name + ": provider 返回 null sprite";
            }
            String id = sprite.getContents().getId().toString();
            String status = id.equals("minecraft:missingno") ? "MISSING（紫黑格）" : "ok";
            return String.format("\n  %s: %s  sprite=%s (%dx%d)",
                name, status, id, sprite.getContents().getWidth(), sprite.getContents().getHeight());
        } catch (Exception e) {
            return "\n  " + name + ": EXCEPTION " + e.getClass().getSimpleName() + ": " + e.getMessage();
        }
    }

    private static int executeSpawn(
        CommandContext<FabricClientCommandSource> ctx,
        double strength,
        int count
    ) {
        Identifier eventId = ctx.getArgument("event_id", Identifier.class);

        ClientPlayerEntity player = MinecraftClient.getInstance().player;
        if (player == null) {
            ctx.getSource().sendError(Text.literal("[bong/spawnp] 本地玩家不存在（世界未加载？）"));
            return 0;
        }
        if (!VfxRegistry.instance().contains(eventId)) {
            ctx.getSource().sendError(Text.literal(
                "[bong/spawnp] 未注册的粒子事件: " + eventId + "（/spawnp list 查看可用）"
            ));
            return 0;
        }

        Vec3d eye = player.getEyePos();
        Vec3d look = player.getRotationVector();
        Vec3d origin = eye.add(look.multiply(DEFAULT_DISTANCE));

        VfxEventPayload.SpawnParticle payload = new VfxEventPayload.SpawnParticle(
            eventId,
            new double[] { origin.x, origin.y, origin.z },
            Optional.of(new double[] { look.x, look.y, look.z }),
            OptionalInt.empty(),
            Optional.of(strength),
            OptionalInt.of(count),
            OptionalInt.of(DEFAULT_DURATION_TICKS)
        );

        boolean ok = new BongVfxParticleBridge().spawnParticle(payload);
        if (!ok) {
            ctx.getSource().sendError(Text.literal("[bong/spawnp] bridge 返回 false（registry 未查到或 client 未就绪）"));
            return 0;
        }
        ctx.getSource().sendFeedback(Text.literal(String.format(Locale.ROOT,
            "[bong/spawnp] 触发 %s @ (%.2f, %.2f, %.2f) strength=%.2f count=%d",
            eventId, origin.x, origin.y, origin.z, strength, count
        )));
        return 1;
    }

    private static CompletableFuture<Suggestions> suggestEventIds(
        CommandContext<FabricClientCommandSource> ctx,
        SuggestionsBuilder builder
    ) {
        String remaining = builder.getRemaining().toLowerCase(Locale.ROOT);
        for (String id : availableEventIds()) {
            if (id.toLowerCase(Locale.ROOT).startsWith(remaining)) {
                builder.suggest(id);
            }
        }
        return builder.buildFuture();
    }
}
