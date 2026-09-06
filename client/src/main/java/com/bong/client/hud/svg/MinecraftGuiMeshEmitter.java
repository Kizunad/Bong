package com.bong.client.hud.svg;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.util.math.MatrixStack;

import java.util.Objects;

/** 将 SVG 三角形以 GUI QUADS 的退化 quad 形式提交给 Minecraft。 */
public final class MinecraftGuiMeshEmitter {
    public static final int MAX_VERTICES = SvgTessellator.MAX_TRIANGLES * 3;

    public void emit(DrawContext context, SvgMesh mesh, int x, int y, float scale, int tint) {
        emit(context, mesh, x, y, scale, scale, tint);
    }

    public void emit(
        DrawContext context,
        SvgMesh mesh,
        int x,
        int y,
        float scaleX,
        float scaleY,
        int tint
    ) {
        Objects.requireNonNull(context, "context");
        Objects.requireNonNull(mesh, "mesh");
        if (!Float.isFinite(scaleX) || scaleX <= 0.0f || !Float.isFinite(scaleY) || scaleY <= 0.0f) {
            throw new IllegalArgumentException("SVG scale 必须是有限正数");
        }
        if (mesh.vertexCount() > MAX_VERTICES) {
            throw new IllegalArgumentException("SVG mesh 顶点数量超过预算: " + MAX_VERTICES);
        }
        MatrixStack.Entry matrix = context.getMatrices().peek();
        VertexConsumer buffer = context.getVertexConsumers().getBuffer(RenderLayer.getGui());
        for (SvgMesh.Triangle triangle : mesh.triangles()) {
            // GUI layer 开启背面剔除，绕序必须与 DrawContext.fill 一致；末点重复形成退化 quad。
            for (int index = 0; index < 4; index++) {
                emitVertex(buffer, matrix, guiQuadVertex(triangle, index), x, y, scaleX, scaleY, tint);
            }
        }
        // DrawContext 由整帧 HUD 统一提交；SVG layer 中途 flush 会打断后续层的顺序。
    }

    static SvgMesh.Vertex guiQuadVertex(SvgMesh.Triangle triangle, int index) {
        Objects.requireNonNull(triangle, "triangle");
        return switch (index) {
            case 0 -> triangle.a();
            case 1 -> triangle.c();
            case 2, 3 -> triangle.b();
            default -> throw new IllegalArgumentException("GUI quad 顶点索引必须在 0 到 3 之间");
        };
    }

    private static void emitVertex(
        VertexConsumer buffer,
        MatrixStack.Entry matrix,
        SvgMesh.Vertex vertex,
        int x,
        int y,
        float scaleX,
        float scaleY,
        int tint
    ) {
        int color = tint(vertex.color(), tint);
        buffer.vertex(
            matrix.getPositionMatrix(),
            x + vertex.x() * scaleX,
            y + vertex.y() * scaleY,
            0.0f
        ).color(
            (color >>> 16) & 0xFF,
            (color >>> 8) & 0xFF,
            color & 0xFF,
            (color >>> 24) & 0xFF
        ).next();
    }

    static int tint(int color, int tint) {
        int a = ((color >>> 24) & 0xFF) * ((tint >>> 24) & 0xFF) / 255;
        int r = ((color >>> 16) & 0xFF) * ((tint >>> 16) & 0xFF) / 255;
        int g = ((color >>> 8) & 0xFF) * ((tint >>> 8) & 0xFF) / 255;
        int b = (color & 0xFF) * (tint & 0xFF) / 255;
        return (a << 24) | (r << 16) | (g << 8) | b;
    }
}
