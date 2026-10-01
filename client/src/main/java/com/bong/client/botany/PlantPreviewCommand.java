package com.bong.client.botany;

import com.mojang.brigadier.arguments.StringArgumentType;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandManager;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.command.CommandSource;
import net.minecraft.text.Text;
import net.minecraft.util.math.Vec3d;

/** 植物模型验收入口：复用世界植物渲染，仅本机可见，不创建采集/种植状态。 */
final class PlantPreviewCommand {
    private static final int PREVIEW_TICKS = 20 * 120;
    private static final String KEY_PREFIX = "plant-preview:";

    private PlantPreviewCommand() {
    }

    static void register() {
        ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) ->
            dispatcher.register(ClientCommandManager.literal("plant-preview")
                .then(ClientCommandManager.literal("spawn")
                    .then(ClientCommandManager.argument("plant", StringArgumentType.word())
                        .suggests((ctx, builder) -> CommandSource.suggestMatching(PlantModelRegistry.geoPlantIds(), builder))
                        .executes(ctx -> spawn(ctx.getSource(), StringArgumentType.getString(ctx, "plant")))))
                .then(ClientCommandManager.literal("all").executes(ctx -> {
                    var source = ctx.getSource();
                    var ids = PlantModelRegistry.geoPlantIds();
                    Vec3d forward = Vec3d.fromPolar(0, source.getPlayer().getYaw());
                    Vec3d right = new Vec3d(-forward.z, 0, forward.x);
                    for (int index = 0; index < ids.size(); index++) {
                        Vec3d position = source.getPosition()
                            .add(forward.multiply(3 + index / 5 * 2.5))
                            .add(right.multiply((index % 5 - 2) * 2.5));
                        show(source, ids.get(index), position);
                    }
                    source.sendFeedback(Text.literal("已预览 " + ids.size() + " 种植物（两分钟后移除）"));
                    return ids.size();
                }))
                .then(ClientCommandManager.literal("clear").executes(ctx -> {
                    for (String id : PlantModelRegistry.geoPlantIds()) {
                        BotanyPlantStageVisualStore.remove(KEY_PREFIX + id);
                    }
                    return 1;
                }))));
    }

    private static int spawn(FabricClientCommandSource source, String id) {
        if (!PlantModelRegistry.geoPlantIds().contains(id)) {
            source.sendError(Text.literal("未找到此植物模型"));
            return 0;
        }
        Vec3d position = source.getPosition().add(Vec3d.fromPolar(0, source.getPlayer().getYaw()).multiply(3));
        show(source, id, position);
        source.sendFeedback(Text.literal("已预览 " + PlantModelRegistry.displayName(id)));
        return 1;
    }

    private static void show(FabricClientCommandSource source, String id, Vec3d position) {
        long now = source.getWorld().getTime();
        // 相同物种复用固定 key，重复执行不会无限堆积预览。
        BotanyPlantStageVisualStore.upsert(new BotanyPlantStageVisual(
            KEY_PREFIX + id, id, PlantGrowthStage.MATURE,
            new double[] { position.x, position.y, position.z }, 0xFFFFFF, 1,
            now + PREVIEW_TICKS, now));
    }
}
