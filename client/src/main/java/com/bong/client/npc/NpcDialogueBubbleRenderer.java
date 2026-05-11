package com.bong.client.npc;

import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderContext;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.render.LightmapTextureManager;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.Entity;
import net.minecraft.util.math.Vec3d;
import org.joml.Matrix4f;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class NpcDialogueBubbleRenderer {
    public static final int MAX_WIDTH_PX = 120;
    public static final int MAX_LINES = 3;
    private static final double FULL_ALPHA_DISTANCE = 15.0;
    private static final double HIDDEN_DISTANCE = 25.0;
    private static final float SCALE = 0.025F;
    private static final int TEXT_COLOR = 0xF0F0F0;
    private static final Map<Integer, Bubble> ACTIVE = new ConcurrentHashMap<>();

    private NpcDialogueBubbleRenderer() {
    }

    public static void register() {
        WorldRenderEvents.AFTER_ENTITIES.register(NpcDialogueBubbleRenderer::render);
    }

    public static void show(Bubble bubble) {
        if (bubble == null || bubble.entityId() < 0 || bubble.text().isBlank()) {
            return;
        }
        ACTIVE.put(bubble.entityId(), bubble);
    }

    public static void clear() {
        ACTIVE.clear();
    }

    public static void clearForTests() {
        clear();
    }

    public static List<Bubble> snapshot(long nowMillis) {
        ACTIVE.entrySet().removeIf(entry -> entry.getValue().expired(nowMillis));
        return ACTIVE.values().stream()
            .sorted(Comparator.comparingInt(Bubble::entityId))
            .toList();
    }

    public static int durationTicksForText(String text) {
        int chars = text == null ? 0 : text.codePointCount(0, text.length());
        double seconds = Math.max(3.0, Math.min(6.0, chars * 0.15));
        return (int) Math.round(seconds * 20.0);
    }

    public static int alphaForDistance(double distanceBlocks) {
        if (!Double.isFinite(distanceBlocks) || distanceBlocks >= HIDDEN_DISTANCE) {
            return 0;
        }
        if (distanceBlocks <= FULL_ALPHA_DISTANCE) {
            return 255;
        }
        double ratio = (HIDDEN_DISTANCE - distanceBlocks) / (HIDDEN_DISTANCE - FULL_ALPHA_DISTANCE);
        return Math.max(0, Math.min(255, (int) Math.round(255.0 * ratio)));
    }

    public static boolean hiddenDuringDialogueScreen(Screen currentScreen) {
        return currentScreen instanceof NpcDialogueScreen;
    }

    public static int backgroundColor(String archetype, String bubbleType, int alpha) {
        int base = switch (archetype == null ? "" : archetype) {
            case "rogue" -> 0x8B7355;
            case "guardian_relic" -> 0x8B6914;
            case "disciple" -> 0x6B3FA0;
            case "zombie", "daoxiang", "zhinian", "fuya" -> 0xD0D0D0;
            case "commoner" -> 0xC4A35A;
            default -> 0x5F8A5F;
        };
        int effectiveAlpha = "memory".equals(bubbleType) ? Math.min(alpha, 220) : alpha;
        if ("daoxiang".equals(archetype) || "zhinian".equals(archetype) || "fuya".equals(archetype)) {
            effectiveAlpha = Math.min(effectiveAlpha, 153);
        }
        return (Math.max(0, Math.min(255, effectiveAlpha)) << 24) | base;
    }

    public static List<String> wrapLines(String text, int maxWidth, WidthMeasurer measurer) {
        if (text == null || text.isBlank()) {
            return List.of();
        }
        WidthMeasurer safeMeasurer = measurer == null ? value -> value.length() * 6 : measurer;
        List<String> lines = new ArrayList<>();
        StringBuilder current = new StringBuilder();
        for (int offset = 0; offset < text.length(); ) {
            int codePoint = text.codePointAt(offset);
            String next = new String(Character.toChars(codePoint));
            String candidate = current + next;
            if (!current.isEmpty() && safeMeasurer.measure(candidate) > maxWidth) {
                lines.add(current.toString());
                current.setLength(0);
                if (lines.size() == MAX_LINES) {
                    break;
                }
            }
            current.append(next);
            offset += Character.charCount(codePoint);
        }
        if (!current.isEmpty() && lines.size() < MAX_LINES) {
            lines.add(current.toString());
        }
        if (lines.size() == MAX_LINES && safeMeasurer.measure(lines.get(MAX_LINES - 1)) > maxWidth) {
            lines.set(MAX_LINES - 1, trimToWidth(lines.get(MAX_LINES - 1), maxWidth, safeMeasurer));
        }
        return List.copyOf(lines);
    }

    private static String trimToWidth(String text, int maxWidth, WidthMeasurer measurer) {
        String suffix = "...";
        StringBuilder out = new StringBuilder();
        for (int offset = 0; offset < text.length(); ) {
            int codePoint = text.codePointAt(offset);
            String candidate = out + new String(Character.toChars(codePoint)) + suffix;
            if (measurer.measure(candidate) > maxWidth) {
                break;
            }
            out.appendCodePoint(codePoint);
            offset += Character.charCount(codePoint);
        }
        return out + suffix;
    }

    private static void render(WorldRenderContext context) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client.world == null || client.player == null || hiddenDuringDialogueScreen(client.currentScreen)) {
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

        for (Bubble bubble : snapshot(nowMillis)) {
            Entity entity = client.world.getEntityById(bubble.entityId());
            if (entity == null) {
                continue;
            }
            double distance = client.player.distanceTo(entity);
            int alpha = Math.min(alphaForDistance(distance), bubble.alphaAt(nowMillis));
            if (alpha <= 0) {
                continue;
            }
            List<String> lines = wrapLines(bubble.text(), MAX_WIDTH_PX, textRenderer::getWidth);
            if (lines.isEmpty()) {
                continue;
            }
            Vec3d pos = entity.getLerpedPos(context.tickDelta()).add(0.0, entity.getHeight() + 0.75, 0.0);
            matrices.push();
            matrices.translate(pos.x - camera.x, pos.y - camera.y, pos.z - camera.z);
            matrices.multiply(context.camera().getRotation());
            matrices.scale(-SCALE, -SCALE, SCALE);
            Matrix4f matrix = matrices.peek().getPositionMatrix();
            int bg = backgroundColor(bubble.archetype(), bubble.bubbleType(), alpha);
            int textColor = (alpha << 24) | TEXT_COLOR;
            for (int i = 0; i < lines.size(); i++) {
                String line = lines.get(i);
                float x = -textRenderer.getWidth(line) / 2.0F;
                textRenderer.draw(
                    line,
                    x,
                    i * 10.0F,
                    textColor,
                    false,
                    matrix,
                    consumers,
                    TextRenderer.TextLayerType.SEE_THROUGH,
                    bg,
                    LightmapTextureManager.MAX_LIGHT_COORDINATE
                );
            }
            matrices.pop();
        }
    }

    @FunctionalInterface
    public interface WidthMeasurer {
        int measure(String text);
    }

    public record Bubble(
        int entityId,
        String text,
        String bubbleType,
        String archetype,
        long durationMillis,
        long shownAtMillis
    ) {
        public Bubble {
            text = text == null ? "" : text.trim();
            bubbleType = bubbleType == null || bubbleType.isBlank() ? "greeting" : bubbleType.trim();
            archetype = archetype == null || archetype.isBlank() ? "unknown" : archetype.trim();
            durationMillis = Math.max(3_000L, Math.min(6_000L, durationMillis));
            shownAtMillis = Math.max(0L, shownAtMillis);
        }

        boolean expired(long nowMillis) {
            return Math.max(0L, nowMillis) - shownAtMillis >= durationMillis;
        }

        int alphaAt(long nowMillis) {
            long age = Math.max(0L, nowMillis) - shownAtMillis;
            long fadeMillis = 300L;
            if (age < fadeMillis) {
                return (int) Math.round(255.0 * age / fadeMillis);
            }
            long remaining = durationMillis - age;
            if (remaining < fadeMillis) {
                return Math.max(0, (int) Math.round(255.0 * remaining / fadeMillis));
            }
            return 255;
        }
    }
}
