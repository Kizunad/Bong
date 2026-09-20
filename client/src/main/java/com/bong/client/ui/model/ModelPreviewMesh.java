package com.bong.client.ui.model;

import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.math.Box;

import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;

/** 先收集本帧真实顶点再取景；同一份网格可使用原材质或统一灰模材质绘制。 */
final class ModelPreviewMesh implements VertexConsumerProvider {
    private final Map<RenderLayer, Part> parts = new LinkedHashMap<>();
    private double minX, minY, minZ, maxX, maxY, maxZ;
    private boolean empty;

    ModelPreviewMesh() { clear(); }

    void clear() {
        parts.values().forEach(part -> part.size = 0);
        minX = minY = minZ = Double.POSITIVE_INFINITY;
        maxX = maxY = maxZ = Double.NEGATIVE_INFINITY;
        empty = true;
    }

    Box bounds() { return empty ? null : new Box(minX, minY, minZ, maxX, maxY, maxZ); }

    @Override public VertexConsumer getBuffer(RenderLayer layer) { return parts.computeIfAbsent(layer, ignored -> new Part()); }

    void draw(MatrixStack matrices, VertexConsumerProvider buffers, RenderLayer clay) {
        var transform = matrices.peek();
        for (var entry : parts.entrySet()) {
            var part = entry.getValue();
            if (part.size == 0) continue;
            var buffer = buffers.getBuffer(clay == null ? entry.getKey() : clay);
            for (int i = 0; i < part.size; i += 14) {
                var d = part.data;
                buffer.vertex(transform.getPositionMatrix(), d[i], d[i + 1], d[i + 2])
                    .color(clay == null ? d[i + 3] : .67f, clay == null ? d[i + 4] : .67f,
                        clay == null ? d[i + 5] : .67f, clay == null ? d[i + 6] : 1)
                    .texture(d[i + 7], d[i + 8])
                    .overlay(clay == null ? Float.floatToRawIntBits(d[i + 9]) : OverlayTexture.DEFAULT_UV)
                    .light(Float.floatToRawIntBits(d[i + 10]))
                    .normal(transform.getNormalMatrix(), d[i + 11], d[i + 12], d[i + 13]).next();
            }
        }
    }

    private final class Part implements VertexConsumer {
        private float[] data = new float[14 * 128];
        private final float[] vertex = new float[14];
        private int size;

        @Override public VertexConsumer vertex(double x, double y, double z) {
            vertex[0] = (float) x; vertex[1] = (float) y; vertex[2] = (float) z;
            return this;
        }
        @Override public VertexConsumer color(int r, int g, int b, int a) {
            vertex[3] = r / 255f; vertex[4] = g / 255f; vertex[5] = b / 255f; vertex[6] = a / 255f;
            return this;
        }
        @Override public VertexConsumer texture(float u, float v) { vertex[7] = u; vertex[8] = v; return this; }
        @Override public VertexConsumer overlay(int u, int v) { vertex[9] = Float.intBitsToFloat(u | v << 16); return this; }
        @Override public VertexConsumer light(int u, int v) { vertex[10] = Float.intBitsToFloat(u | v << 16); return this; }
        @Override public VertexConsumer normal(float x, float y, float z) { vertex[11] = x; vertex[12] = y; vertex[13] = z; return this; }
        @Override public void fixedColor(int r, int g, int b, int a) { color(r, g, b, a); }
        @Override public void unfixColor() {}
        @Override public void next() {
            if (!Float.isFinite(vertex[0]) || !Float.isFinite(vertex[1]) || !Float.isFinite(vertex[2])) {
                throw new IllegalStateException("模型包含无效顶点");
            }
            if (size + 14 > data.length) data = Arrays.copyOf(data, data.length * 2);
            System.arraycopy(vertex, 0, data, size, 14);
            size += 14;
            minX = Math.min(minX, vertex[0]); maxX = Math.max(maxX, vertex[0]);
            minY = Math.min(minY, vertex[1]); maxY = Math.max(maxY, vertex[1]);
            minZ = Math.min(minZ, vertex[2]); maxZ = Math.max(maxZ, vertex[2]);
            empty = false;
        }
    }
}
