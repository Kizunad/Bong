package com.bong.client.npc;

import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderContext;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.render.LightmapTextureManager;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.Entity;
import net.minecraft.util.math.Vec3d;
import org.joml.Matrix4f;

public final class NpcNametagRenderer {
    private static final double FULL_LABEL_DISTANCE = 20.0;
    private static final double ICON_LABEL_DISTANCE = 40.0;
    private static final float SCALE = 0.025F;
    private static final int COLOR_HOSTILE = 0xE05A47;
    private static final int COLOR_NEUTRAL = 0xC8C8C8;
    private static final int COLOR_FRIENDLY = 0x5DD17A;

    private NpcNametagRenderer() {
    }

    public static void register() {
        WorldRenderEvents.AFTER_ENTITIES.register(NpcNametagRenderer::render);
    }

    public static int colorByReputation(int reputationToPlayer) {
        if (reputationToPlayer < -30) {
            return COLOR_HOSTILE;
        }
        if (reputationToPlayer > 50) {
            return COLOR_FRIENDLY;
        }
        return COLOR_NEUTRAL;
    }

    public static String labelForDistance(NpcMetadata metadata, double distance, String playerRealm) {
        if (metadata == null || distance >= ICON_LABEL_DISTANCE) {
            return "";
        }
        String dangerPrefix = shouldShowDangerWarning(metadata.realm(), playerRealm) ? "⚠ " : "";
        if (distance >= FULL_LABEL_DISTANCE) {
            return dangerPrefix + archetypeIcon(metadata.archetype());
        }
        return dangerPrefix + "[" + metadata.displayName() + "]";
    }

    public static boolean shouldShowDangerWarning(String npcRealm, String playerRealm) {
        return realmRank(npcRealm) - realmRank(playerRealm) >= 2;
    }

    private static void render(WorldRenderContext context) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client.world == null || client.player == null) {
            return;
        }
        VertexConsumerProvider consumers = context.consumers();
        MatrixStack matrices = context.matrixStack();
        if (consumers == null || matrices == null) {
            return;
        }
        Vec3d camera = context.camera().getPos();
        TextRenderer textRenderer = client.textRenderer;
        String playerRealm = playerRealmLabel();

        for (Entity entity : client.world.getEntities()) {
            NpcMetadata metadata = NpcMetadataStore.get(entity.getId());
            if (metadata == null) {
                continue;
            }
            double distance = client.player.distanceTo(entity);
            String label = labelForDistance(metadata, distance, playerRealm);
            if (label.isEmpty()) {
                continue;
            }
            Vec3d pos = entity.getLerpedPos(context.tickDelta()).add(0.0, entity.getHeight() + 0.45, 0.0);
            matrices.push();
            matrices.translate(pos.x - camera.x, pos.y - camera.y, pos.z - camera.z);
            matrices.multiply(context.camera().getRotation());
            matrices.scale(-SCALE, -SCALE, SCALE);
            Matrix4f matrix = matrices.peek().getPositionMatrix();
            float x = -textRenderer.getWidth(label) / 2.0F;
            textRenderer.draw(
                label,
                x,
                0.0F,
                colorByReputation(metadata.reputationToPlayer()),
                false,
                matrix,
                consumers,
                TextRenderer.TextLayerType.SEE_THROUGH,
                0x40000000,
                LightmapTextureManager.MAX_LIGHT_COORDINATE
            );
            matrices.pop();
        }
    }

    private static String playerRealmLabel() {
        var state = com.bong.client.state.PlayerStateStore.snapshot();
        return state == null ? "引气" : state.realm();
    }

    private static String archetypeIcon(String archetype) {
        return switch (archetype) {
            case "rogue" -> "散";
            case "commoner" -> "凡";
            case "disciple" -> "宗";
            case "beast" -> "兽";
            default -> "人";
        };
    }

    private static int realmRank(String realm) {
        return switch (realm) {
            case "引灵" -> 1;
            case "凝脉" -> 2;
            case "固元" -> 3;
            case "化神" -> 4;
            case "渡虚" -> 5;
            default -> 0;
        };
    }
}
