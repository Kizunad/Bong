package com.bong.client.fauna;

import com.mojang.brigadier.arguments.StringArgumentType;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandManager;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.minecraft.command.CommandSource;
import net.minecraft.text.Text;
import net.minecraft.util.math.Vec3d;

import java.util.ArrayList;
import java.util.List;

/** 客户端模型验收：spawn 物种、play 动作（支持 Tab）、disguise/clear，不生成服务端生物。 */
public final class FaunaPreviewCommand {
    private static final List<FaunaEntity> PREVIEWS = new ArrayList<>();
    private static FaunaEntity selected;
    private static int nextId = -300_000;

    private FaunaPreviewCommand() {}

    public static void register() {
        ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) -> {
            var spawn = ClientCommandManager.literal("spawn");
            for (FaunaVisualKind kind : FaunaVisualKind.values()) {
                spawn.then(ClientCommandManager.literal(kind.path()).executes(ctx -> {
                    var player = ctx.getSource().getPlayer();
                    var world = ctx.getSource().getWorld();
                    Vec3d direction = Vec3d.fromPolar(0, player.getYaw());
                    Vec3d position = player.getPos().add(direction.multiply(4));
                    FaunaEntity entity = FaunaEntities.type(kind).create(world);
                    if (entity == null) return 0;
                    entity.refreshPositionAndAngles(position.x, position.y, position.z, player.getYaw() + 180, 0);
                    entity.setId(nextId--);
                    world.addEntity(entity.getId(), entity);
                    PREVIEWS.add(entity);
                    selected = entity;
                    ctx.getSource().sendFeedback(Text.literal("已预览 " + kind.path() + "；/fauna-preview play <动作> 支持 Tab"));
                    return 1;
                }));
            }
            dispatcher.register(ClientCommandManager.literal("fauna-preview")
                .then(spawn)
                .then(ClientCommandManager.literal("play")
                    .then(ClientCommandManager.argument("animation", StringArgumentType.word())
                        .suggests((ctx, builder) -> CommandSource.suggestMatching(
                            selected == null ? List.of() : FaunaAnimations.profiles(selected.visualKind()).stream()
                                .flatMap(profile -> profile.clips().keySet().stream()).sorted().toList(), builder))
                        .executes(ctx -> {
                            if (selected == null || selected.isRemoved()) {
                                ctx.getSource().sendError(Text.literal("先用 /fauna-preview spawn <物种> 生成预览"));
                                return 0;
                            }
                            String name = StringArgumentType.getString(ctx, "animation");
                            for (var profile : FaunaAnimations.profiles(selected.visualKind())) {
                                var clip = profile.find(name);
                                if (clip != null) {
                                    selected.triggerAction(clip.name(), clip.loop() ? 600 : clip.ticks());
                                    ctx.getSource().sendFeedback(Text.literal("播放 " + clip.name()));
                                    return 1;
                                }
                            }
                            ctx.getSource().sendError(Text.literal("该模型没有此动作；用 Tab 查看完整列表"));
                            return 0;
                        })))
                .then(ClientCommandManager.literal("disguise").executes(ctx -> {
                    if (selected == null || selected.visualKind() != FaunaVisualKind.ASH_SPIDER) return 0;
                    selected.togglePreviewDisguise();
                    return 1;
                }))
                .then(ClientCommandManager.literal("clear").executes(ctx -> {
                    PREVIEWS.forEach(FaunaEntity::discard);
                    PREVIEWS.clear();
                    selected = null;
                    return 1;
                })));
        });
    }

    public static void clearOnDisconnect() {
        PREVIEWS.clear();
        selected = null;
        nextId = -300_000;
    }
}
