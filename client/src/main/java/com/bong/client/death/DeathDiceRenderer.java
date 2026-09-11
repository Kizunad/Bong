package com.bong.client.death;

import com.bong.client.combat.store.DeathStateStore;
import com.mojang.blaze3d.systems.RenderSystem;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.render.BufferBuilder;
import net.minecraft.client.render.BufferRenderer;
import net.minecraft.client.render.GameRenderer;
import net.minecraft.client.render.Tessellator;
import net.minecraft.client.render.VertexFormat;
import net.minecraft.client.render.VertexFormats;
import net.minecraft.util.Identifier;
import org.joml.Matrix4f;
import org.joml.Vector3f;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/** 原生 GUI 内的立体骰子演出；结果只取服务端，旋转与落桌均为确定时间轴。 */
public final class DeathDiceRenderer {
    private static final Identifier TABLE = new Identifier("bong-client", "textures/gui/death/void-table.png");
    private static final Identifier FACES = new Identifier("bong-client", "textures/gui/death/dice-faces.png");
    private static final float[][] VERTICES = {
        {-1,-1,-1}, {1,-1,-1}, {1,1,-1}, {-1,1,-1},
        {-1,-1,1}, {1,-1,1}, {1,1,1}, {-1,1,1}
    };
    private static final int[][] FACES_VERTICES = {
        {4,5,6,7}, {1,0,3,2}, {0,4,7,3}, {5,1,2,6}, {0,1,5,4}, {7,6,2,3}
    };

    private DeathDiceRenderer() {}

    public static boolean rolling(DeathStateStore.State state) {
        return !state.canReincarnate() && !state.canTerminate() && state.cinematic().active()
            && (state.cinematic().roll().result() == DeathCinematicState.RollResult.SURVIVE
                || state.cinematic().roll().result() == DeathCinematicState.RollResult.FALL);
    }

    public static double progress(DeathStateStore.State state, long now) {
        return Math.max(0, Math.min(1, elapsedTicks(state, now) / state.cinematic().totalDurationTicks()));
    }

    private static double elapsedTicks(DeathStateStore.State state, long now) {
        var cinematic = state.cinematic();
        return cinematic.totalElapsedTicks() + Math.max(0, now - cinematic.receivedAtMillis()) / 50.0;
    }

    public static void render(DrawContext context, DeathStateStore.State state, long now, int x, int y, int width, int height) {
        context.enableScissor(x, y, x + width, y + height);
        try {
            boolean rolling = rolling(state);
            double progress = rolling ? progress(state, now) : 0;
            double flight = Math.max(0, Math.min(1, (progress - 0.16) / 0.38));
            float tableScale = Math.min(width / 1024f, height / 640f);
            float tableY = y + height - 640 * tableScale;
            drawTable(context, x, tableY, width, tableScale, rolling ? (float) (0.45 + 0.55 * flight) : 1);
            boolean big = state.cinematic().roll().result() != DeathCinematicState.RollResult.FALL;
            float unit = Math.min(height, width);
            float rx = -0.48f;
            float ry = 0.58f;
            float rz = 0;
            float radius = unit * 0.09f;
            float cx = x + width * 0.5f;
            float lift = 0;
            if (rolling && progress < 0.16) {
                double tension = Math.sin(progress / 0.16 * Math.PI);
                radius = (float) (unit * (0.19 + tension * 0.01));
                cx -= (float) (unit * (0.16 + tension * 0.035));
                rx -= (float) (tension * 0.22);
                rz = (float) (-tension * 0.18);
            } else if (rolling && progress < 0.54) {
                // 透视缩小配合抛物线；整圈翻滚在接触桌面时回到可读点数的角度。
                double turn = 1 - Math.pow(1 - flight, 2);
                radius = (float) (unit * (0.19 - flight * 0.10));
                cx += (float) (unit * (-0.16 * (1 - flight) + 0.10 * Math.sin(flight * Math.PI)));
                rx += (float) (turn * Math.PI * 4);
                ry += (float) (turn * Math.PI * 6);
                rz = (float) (Math.sin(flight * Math.PI) * 0.6);
            } else if (rolling && progress < 0.80) {
                double bounce = (progress - 0.54) / 0.26;
                lift = (float) (unit * (bounce < 0.65
                    ? 0.10 * Math.sin(bounce / 0.65 * Math.PI)
                    : 0.025 * Math.sin((bounce - 0.65) / 0.35 * Math.PI)));
                cx += (float) (unit * 0.045 * Math.sin(bounce * Math.PI));
                rz = (float) (Math.sin(bounce * Math.PI * 3) * (1 - bounce) * 0.24);
                rx += (float) (Math.sin(bounce * Math.PI * 2) * (1 - bounce) * 0.16);
            } else {
                double seconds = rolling
                    ? Math.max(0, elapsedTicks(state, now) - state.cinematic().totalDurationTicks() * 0.80) / 20
                    : now / 1000.0;
                double rise = rolling ? Math.min(1, seconds / 0.24) : 1;
                lift = (float) (unit * rise * (0.023 + 0.015 * Math.sin(seconds * Math.PI * 1.1)));
            }
            Vector3f[] points = rotatedVertices(rx, ry, rz);
            float floorY = tableY + 640 * tableScale * 0.46f;
            float bottom = 0;
            for (Vector3f point : points) bottom = Math.max(bottom, point.y * 5 / (5 - point.z));
            float cy = floorY - radius * bottom - lift;
            if (rolling && progress < 0.16) {
                cy = (float) (y + height * (0.52 + 0.04 * Math.sin(progress / 0.16 * Math.PI)));
            } else if (rolling && progress < 0.54) {
                cy = (float) ((y + height * 0.52) * (1 - flight) + cy * flight
                    - height * 0.16 * 4 * flight * (1 - flight));
            }
            float proximity = rolling && progress < 0.54 ? (float) flight : Math.max(0.25f, 1 - lift / (unit * 0.13f));
            drawContactShadow(context, cx, floorY, unit * (0.10f + 0.03f * proximity), proximity);
            context.draw();
            drawCube(context, cx, cy, radius, points, big);
            if (rolling && progress >= 0.80) {
                String result = big ? "大 · 再续此身" : "小 · 此生终结";
                context.drawCenteredTextWithShadow(MinecraftClient.getInstance().textRenderer, result,
                    x + width / 2, y + height - 16, big ? 0xFFB5D5BB : 0xFFE4A798);
            }
        } finally {
            context.disableScissor();
        }
    }

    private static void drawTable(DrawContext context, int x, float y, int width, float scale, float opacity) {
        var matrices = context.getMatrices();
        matrices.push();
        try {
            matrices.translate(x + (width - 1024 * scale) / 2, y, 0);
            matrices.scale(scale, scale, 1);
            RenderSystem.enableBlend();
            RenderSystem.defaultBlendFunc();
            RenderSystem.setShaderColor(1, 1, 1, opacity);
            context.drawTexture(TABLE, 0, 0, 0, 0, 1024, 640, 1024, 640);
            context.draw();
        } finally {
            RenderSystem.setShaderColor(1, 1, 1, 1);
            matrices.pop();
        }
    }

    private static void drawContactShadow(DrawContext context, float x, float y, float radius, float opacity) {
        int halfHeight = Math.max(2, Math.round(radius * 0.23f));
        int color = Math.round(opacity * 110) << 24;
        for (int row = -halfHeight; row <= halfHeight; row++) {
            int halfWidth = Math.round(radius * (float) Math.sqrt(1 - Math.pow(row / (double) halfHeight, 2)));
            context.fill(Math.round(x) - halfWidth, Math.round(y) + row,
                Math.round(x) + halfWidth, Math.round(y) + row + 1, color);
        }
    }

    private static Vector3f[] rotatedVertices(float rx, float ry, float rz) {
        Matrix4f rotation = new Matrix4f().rotateZ(rz).rotateY(ry).rotateX(rx);
        Vector3f[] points = new Vector3f[8];
        for (int i = 0; i < points.length; i++) points[i] = rotation.transformPosition(new Vector3f(VERTICES[i]));
        return points;
    }

    private static void drawCube(DrawContext context, float cx, float cy, float radius, Vector3f[] points, boolean big) {
        List<Integer> faces = new ArrayList<>();
        for (int i = 0; i < 6; i++) faces.add(i);
        faces.sort(Comparator.comparingDouble(face -> depth(points, FACES_VERTICES[face])));
        RenderSystem.setShader(GameRenderer::getPositionTexColorProgram);
        RenderSystem.setShaderTexture(0, FACES);
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        RenderSystem.disableCull();
        try {
            BufferBuilder buffer = Tessellator.getInstance().getBuffer();
            buffer.begin(VertexFormat.DrawMode.QUADS, VertexFormats.POSITION_TEXTURE_COLOR);
            Matrix4f matrix = context.getMatrices().peek().getPositionMatrix();
            for (int face : faces) {
                int[] indices = FACES_VERTICES[face];
                Vector3f normal = new Vector3f(points[indices[1]]).sub(points[indices[0]])
                    .cross(new Vector3f(points[indices[2]]).sub(points[indices[0]])).normalize();
                int light = (int) (190 + 65 * Math.max(0, normal.dot(new Vector3f(-0.3f, -0.6f, 0.7f))));
                // 点数看朝上的一面；每对相反面的点数之和始终为 7。
                int tile = switch (face) {
                    case 0 -> 1;
                    case 1 -> 4;
                    case 2 -> 2;
                    case 3 -> 3;
                    case 4 -> big ? 5 : 0;
                    default -> big ? 0 : 5;
                };
                for (int corner = 0; corner < 4; corner++) {
                    Vector3f point = points[indices[corner]];
                    float perspective = 5 / (5 - point.z);
                    float u = (tile + (corner == 1 || corner == 2 ? 1 : 0)) / 6f;
                    float v = corner >= 2 ? 1 : 0;
                    buffer.vertex(matrix, cx + point.x * radius * perspective, cy + point.y * radius * perspective, 0)
                        .texture(u, v).color(light, light, light, 255).next();
                }
            }
            BufferRenderer.drawWithGlobalProgram(buffer.end());
        } finally {
            RenderSystem.enableCull();
        }
    }

    private static double depth(Vector3f[] points, int[] face) {
        double depth = 0;
        for (int index : face) depth += points[index].z;
        return depth;
    }
}
