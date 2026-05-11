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

public final class NpcMoodIcon {
    private static final float SCALE = 0.022F;
    private static final int ALERT_COLOR = 0xFFE2C84A;
    private static final int HOSTILE_COLOR = 0xFFFF4040;
    private static final int FEARFUL_COLOR = 0xFFA9B8C8;

    private NpcMoodIcon() {
    }

    public static void register() {
        WorldRenderEvents.AFTER_ENTITIES.register(NpcMoodIcon::render);
    }

    public static String texturePath(String mood) {
        return switch (normalizeMood(mood)) {
            case "alert" -> "bong-client:textures/gui/npc/mood_alert.png";
            case "hostile" -> "bong-client:textures/gui/npc/mood_hostile.png";
            case "fearful" -> "bong-client:textures/gui/npc/mood_fearful.png";
            default -> "";
        };
    }

    public static int iconSize(String mood) {
        return "hostile".equals(normalizeMood(mood)) ? 14 : 12;
    }

    public static int alphaAt(long transitionStartedAtMillis, long nowMillis) {
        long age = Math.max(0L, nowMillis - Math.max(0L, transitionStartedAtMillis));
        if (age >= 300L) {
            return 255;
        }
        return Math.max(0, Math.min(255, (int) Math.round(255.0 * age / 300.0)));
    }

    public static int transitionColor(String fromMood, String toMood, long transitionStartedAtMillis, long nowMillis) {
        int from = colorFor(fromMood);
        int to = colorFor(toMood);
        double t = Math.max(0.0, Math.min(1.0, (nowMillis - transitionStartedAtMillis) / 200.0));
        return lerpColor(from, to, t);
    }

    public static double fearfulShakeOffset(long nowMillis) {
        return Math.sin(nowMillis / 55.0) * 0.5;
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
        long nowMillis = System.currentTimeMillis();
        for (NpcMoodState mood : NpcMoodStore.snapshot()) {
            if ("neutral".equals(mood.mood())) {
                continue;
            }
            Entity entity = client.world.getEntityById(mood.entityId());
            if (entity == null || client.player.distanceTo(entity) > 32.0) {
                continue;
            }
            String glyph = switch (mood.mood()) {
                case "hostile" -> "!!";
                case "fearful" -> "?!";
                default -> "!";
            };
            Vec3d pos = entity.getLerpedPos(context.tickDelta()).add(0.0, entity.getHeight() + 0.95, 0.0);
            matrices.push();
            matrices.translate(pos.x - camera.x, pos.y - camera.y, pos.z - camera.z);
            matrices.multiply(context.camera().getRotation());
            matrices.scale(-SCALE, -SCALE, SCALE);
            if (mood.fearful()) {
                matrices.translate(fearfulShakeOffset(nowMillis), 0.0, 0.0);
            }
            Matrix4f matrix = matrices.peek().getPositionMatrix();
            float x = -textRenderer.getWidth(glyph) / 2.0F;
            textRenderer.draw(
                glyph,
                x,
                0.0F,
                colorFor(mood.mood()),
                false,
                matrix,
                consumers,
                TextRenderer.TextLayerType.SEE_THROUGH,
                0x00000000,
                LightmapTextureManager.MAX_LIGHT_COORDINATE
            );
            matrices.pop();
        }
    }

    private static String normalizeMood(String mood) {
        return mood == null ? "neutral" : mood.trim().toLowerCase(java.util.Locale.ROOT);
    }

    private static int colorFor(String mood) {
        return switch (normalizeMood(mood)) {
            case "hostile" -> HOSTILE_COLOR;
            case "fearful" -> FEARFUL_COLOR;
            case "alert" -> ALERT_COLOR;
            default -> 0x00000000;
        };
    }

    private static int lerpColor(int from, int to, double t) {
        int a = lerp((from >>> 24) & 0xFF, (to >>> 24) & 0xFF, t);
        int r = lerp((from >>> 16) & 0xFF, (to >>> 16) & 0xFF, t);
        int g = lerp((from >>> 8) & 0xFF, (to >>> 8) & 0xFF, t);
        int b = lerp(from & 0xFF, to & 0xFF, t);
        return (a << 24) | (r << 16) | (g << 8) | b;
    }

    private static int lerp(int from, int to, double t) {
        return (int) Math.round(from + (to - from) * t);
    }
}
