package com.bong.client.death;

import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.component.ButtonComponent;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.render.BufferBuilder;
import net.minecraft.client.render.BufferRenderer;
import net.minecraft.client.render.GameRenderer;
import net.minecraft.client.render.Tessellator;
import net.minecraft.client.render.VertexFormat;
import net.minecraft.client.render.VertexFormats;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import net.minecraft.util.Util;

/** 域外背景、独立世界泡帧动画与偶发游光；仅随渲染时钟推进，无常驻回调。 */
public final class DeathBackdrop {
    private static final Identifier VOID = new Identifier("bong-client", "textures/gui/death/interworld-void.png");
    private static final Identifier WISPS = new Identifier("bong-client", "textures/gui/death/star-wisps.png");
    private static final Identifier TITLE_FONT = new Identifier("bong", "menu-title");
    private static final long STARTED_AT = Util.getMeasuringTimeMs();
    public static final int INK_COLOR = 0xFFD5DBD5;
    public static final int REBIRTH_COLOR = 0xFFADC8B9;
    public static final int ENDING_COLOR = 0xFFD5A18D;
    private static final WorldLayer[] WORLDS_LAYOUT = {
        new WorldLayer("primordial", 0.08, 0.28, 0.58, 0.55f, 0),
        new WorldLayer("technological", 0.91, 0.35, 0.52, 0.50f, 2.1),
        new WorldLayer("cultivation", 0.15, 0.84, 0.40, 0.48f, 4.2),
        new WorldLayer("psionic", 0.68, 0.04, 0.37, 0.42f, 1.2),
        new WorldLayer("high-magic", 0.86, 0.87, 0.42, 0.48f, 3.3)
    };

    private DeathBackdrop() {}

    public static void render(DrawContext context, int width, int height, int accent) {
        double seconds = (Util.getMeasuringTimeMs() - STARTED_AT) / 1000.0;
        context.fill(0, 0, width, height, 0xFF060809);
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        try {
            float scale = Math.max(width / 1536f, height / 1024f);
            texture(context, VOID, (width - 1536 * scale) / 2, (height - 1024 * scale) / 2,
                1536 * scale, 1024 * scale, 0, 0, 1536, 1024, 1536, 1024, 1);
            for (WorldLayer world : WORLDS_LAYOUT) {
                float size = (float) (height * world.size * (1 + 0.045 * Math.sin(seconds * 0.24 + world.phase)));
                float x = (float) (width * world.x + Math.sin(seconds * 0.075 + world.phase) * height * 0.025);
                float y = (float) (height * world.y + Math.cos(seconds * 0.095 + world.phase) * height * 0.018);
                // 每种世界各自往返播放；局部透隐独立推进，保留原图细节和 alpha。
                double position = (seconds * 0.32 + world.phase) % 6;
                if (position > 3) position = 6 - position;
                int frame = (int) position;
                float blend = (float) (position - frame);
                float visibility = (float) (0.25 + 0.75 * (0.5 + 0.5 * Math.sin(seconds * 0.26 + world.phase)));
                drawWorldFrame(context, world, frame, x - size / 2, y - size / 2, size,
                    world.alpha * visibility * (1 - blend), seconds, width);
                if (blend > 0) drawWorldFrame(context, world, (frame + 1) % 4, x - size / 2, y - size / 2, size,
                    world.alpha * visibility * blend, seconds, width);
            }
            drawWisp(context, width, height, seconds);
        } finally {
            RenderSystem.setShaderColor(1, 1, 1, 1);
        }
        context.fill(width - 18, 18, width - 16, 43, accent);
    }

    private static void drawWorldFrame(DrawContext context, WorldLayer world, int frame, float x, float y,
                                       float size, float alpha, double seconds, int viewportWidth) {
        context.draw();
        RenderSystem.setShader(GameRenderer::getPositionTexColorProgram);
        RenderSystem.setShaderTexture(0, world.image);
        BufferBuilder buffer = Tessellator.getInstance().getBuffer();
        buffer.begin(VertexFormat.DrawMode.QUADS, VertexFormats.POSITION_TEXTURE_COLOR);
        var matrix = context.getMatrices().peek().getPositionMatrix();
        int divisions = 16;
        for (int row = 0; row < divisions; row++) {
            for (int column = 0; column < divisions; column++) {
                for (int corner = 0; corner < 4; corner++) {
                    float u = (column + (corner >= 2 ? 1 : 0)) / (float) divisions;
                    float v = (row + (corner == 1 || corner == 2 ? 1 : 0)) / (float) divisions;
                    double veil = 0.5 + 0.28 * Math.sin(u * 6.5 + v * 3.5 - seconds * 0.24 + world.phase)
                        + 0.22 * Math.cos(v * 8 - u * 2 + seconds * 0.18 + world.phase * 2);
                    double visible = Math.max(0, Math.min(1, (veil - 0.32) / 0.38));
                    visible = visible * visible * (3 - 2 * visible);
                    // 接近中央文字时减弱背景；窄屏也不让世界内部灯光盖过控件。
                    double distance = Math.abs(x + u * size - viewportWidth * 0.5) / Math.min(215, viewportWidth * 0.42);
                    double centerFade = 0.50 + 0.50 * Math.min(1, distance * distance);
                    buffer.vertex(matrix, x + u * size, y + v * size, 0)
                        .texture((frame % 2 + u) / 2, (frame / 2 + v) / 2)
                        .color(205, 205, 205, (int) (255 * alpha * visible * centerFade)).next();
                }
            }
        }
        BufferRenderer.drawWithGlobalProgram(buffer.end());
    }

    private static void drawWisp(DrawContext context, int width, int height, double seconds) {
        double cycle = Math.floor(seconds / 13);
        double progress = (seconds % 13) / 4.5;
        if (progress >= 1) return;
        boolean reverse = ((long) cycle & 1) != 0;
        float x = (float) (width * (reverse ? 0.87 - progress * 0.72 : 0.1 + progress * 0.72));
        float y = (float) (height * (0.16 + 0.07 * Math.sin(cycle * 1.7) + progress * 0.09));
        float size = Math.max(28, height * 0.14f);
        var matrices = context.getMatrices();
        matrices.push();
        try {
            matrices.translate(x, y, 0);
            matrices.multiply(new org.joml.Quaternionf().rotateZ(reverse ? (float) Math.PI - 0.12f : 0.12f));
            sprite(context, WISPS, 2, 2, 128, (seconds * 2) % 4, -size / 2, -size / 2, size,
                (float) (0.36 * Math.sin(progress * Math.PI)));
        } finally {
            matrices.pop();
        }
    }

    private static void sprite(DrawContext context, Identifier image, int columns, int rows, int cell,
                               double position, float x, float y, float size, float alpha) {
        int frame = (int) position;
        float blend = (float) (position - frame);
        int next = (frame + 1) % (columns * rows);
        texture(context, image, x, y, size, size, frame % columns * cell, frame / columns * cell,
            cell, cell, columns * cell, rows * cell, alpha * (1 - blend));
        if (blend > 0) texture(context, image, x, y, size, size, next % columns * cell, next / columns * cell,
            cell, cell, columns * cell, rows * cell, alpha * blend);
    }

    private static void texture(DrawContext context, Identifier image, float x, float y, float width, float height,
                                int u, int v, int regionWidth, int regionHeight, int imageWidth, int imageHeight, float alpha) {
        var matrices = context.getMatrices();
        matrices.push();
        try {
            matrices.translate(x, y, 0);
            matrices.scale(width / regionWidth, height / regionHeight, 1);
            RenderSystem.setShaderColor(1, 1, 1, alpha);
            context.drawTexture(image, 0, 0, u, v, regionWidth, regionHeight, imageWidth, imageHeight);
            context.draw();
        } finally {
            RenderSystem.setShaderColor(1, 1, 1, 1);
            matrices.pop();
        }
    }

    private record WorldLayer(Identifier image, double x, double y, double size, float alpha, double phase) {
        private WorldLayer(String name, double x, double y, double size, float alpha, double phase) {
            this(new Identifier("bong-client", "textures/gui/death/world-" + name + ".png"), x, y, size, alpha, phase);
        }
    }

    public static void title(DrawContext context, String text, int centerX, int y) {
        var font = MinecraftClient.getInstance().textRenderer;
        Text title = Text.literal(text).styled(style -> style.withFont(TITLE_FONT));
        var matrices = context.getMatrices();
        matrices.push();
        try {
            matrices.translate(centerX, y, 0);
            matrices.scale(2, 2, 1);
            context.drawText(font, title, -font.getWidth(title) / 2, 0, INK_COLOR, false);
        } finally {
            matrices.pop();
        }
    }

    public static void command(ButtonComponent button, int accent) {
        button.textShadow(false).renderer((context, value, delta) -> {
            boolean focused = value.active && (value.isHovered() || value.isFocused());
            int color = value.active ? accent : 0xFF58615D;
            context.fill(value.x(), value.y(), value.x() + value.width(), value.y() + value.height(),
                focused ? 0xCC24332E : 0x99111816);
            context.fill(value.x(), value.y() + value.height() - 1, value.x() + value.width(), value.y() + value.height(), color);
            if (focused) context.fill(value.x(), value.y() + 5, value.x() + 2, value.y() + value.height() - 5, accent);
        });
    }
}
