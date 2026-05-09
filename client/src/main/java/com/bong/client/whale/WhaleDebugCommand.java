package com.bong.client.whale;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.context.CommandContext;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandManager;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.text.Text;
import net.minecraft.util.math.Vec3d;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Phase B-1 client-only 调试指令：
 * <ul>
 *   <li>{@code /whale-debug spawn} — 在玩家面前 30 格距离 + 上方 10 格 spawn 一只静态客户端 whale，
 *       验证 GeckoLib 渲染管线（geo.json + 贴图 + idle 动画）</li>
 *   <li>{@code /whale-debug clear} — 清掉所有本会话 spawn 的客户端 whale</li>
 * </ul>
 *
 * <p>不联 server。entity 直接 add 到 client world 实体表，networkId 走负数避开 server 范围。
 * Phase B-2 server 接入后此命令会废弃，由 server-side spawn 替代。
 */
public final class WhaleDebugCommand {
    private static final String ROOT = "whale-debug";
    private static final double FORWARD_DISTANCE = 30.0;
    private static final double VERTICAL_OFFSET = 10.0;

    /** 本会话生成的所有客户端 whale，clear 子命令用来全删。 */
    private static final List<WhaleEntity> SPAWNED = new ArrayList<>();
    /** networkId 分配器：从 -10000 起递减，避开 server 实体 ID 区间。 */
    private static final AtomicInteger NEXT_ID = new AtomicInteger(-10_000);

    private WhaleDebugCommand() {}

    public static void register() {
        ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) -> registerTo(dispatcher));
    }

    static void registerTo(CommandDispatcher<FabricClientCommandSource> dispatcher) {
        dispatcher.register(ClientCommandManager.literal(ROOT)
            .then(ClientCommandManager.literal("spawn").executes(WhaleDebugCommand::executeSpawn))
            .then(ClientCommandManager.literal("clear").executes(WhaleDebugCommand::executeClear))
            .executes(WhaleDebugCommand::executeSpawn));
    }

    private static int executeSpawn(CommandContext<FabricClientCommandSource> ctx) {
        MinecraftClient client = MinecraftClient.getInstance();
        ClientPlayerEntity player = client.player;
        ClientWorld world = client.world;
        if (player == null || world == null) {
            ctx.getSource().sendError(Text.literal("[bong/whale-debug] 玩家或世界未就绪"));
            return 0;
        }

        Vec3d look = player.getRotationVector();
        // 仅取水平方向；竖直由 VERTICAL_OFFSET 固定
        Vec3d horiz = new Vec3d(look.x, 0, look.z);
        if (horiz.lengthSquared() < 1.0e-6) {
            horiz = new Vec3d(0, 0, 1);
        } else {
            horiz = horiz.normalize();
        }
        Vec3d origin = player.getPos()
            .add(horiz.multiply(FORWARD_DISTANCE))
            .add(0, VERTICAL_OFFSET, 0);

        WhaleEntity whale = WhaleEntities.whale().create(world);
        if (whale == null) {
            ctx.getSource().sendError(Text.literal("[bong/whale-debug] EntityType.create 返回 null（registry 未就绪？）"));
            return 0;
        }

        // 让鲸朝向玩家：yaw 从 whale 指向 player
        Vec3d toPlayer = player.getPos().subtract(origin);
        float yaw = (float) (Math.toDegrees(Math.atan2(-toPlayer.x, toPlayer.z)));
        whale.refreshPositionAndAngles(origin.x, origin.y, origin.z, yaw, 0.0f);
        whale.setVelocity(Vec3d.ZERO);

        int id = NEXT_ID.getAndDecrement();
        whale.setId(id);
        world.addEntity(id, whale);
        SPAWNED.add(whale);

        ctx.getSource().sendFeedback(Text.literal(String.format(
            "[bong/whale-debug] spawned id=%d at (%.1f, %.1f, %.1f) yaw=%.1f total=%d",
            id, origin.x, origin.y, origin.z, yaw, SPAWNED.size())));
        return 1;
    }

    private static int executeClear(CommandContext<FabricClientCommandSource> ctx) {
        int n = 0;
        for (WhaleEntity w : SPAWNED) {
            w.discard();
            n++;
        }
        SPAWNED.clear();
        ctx.getSource().sendFeedback(Text.literal("[bong/whale-debug] cleared " + n + " whales"));
        return n;
    }
}
