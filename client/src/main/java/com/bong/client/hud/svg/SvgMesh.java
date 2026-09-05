package com.bong.client.hud.svg;

import java.util.List;
import java.util.Objects;

/** SVG tessellation 的不可变三角形网格。 */
public final class SvgMesh {
    private final List<Triangle> triangles;
    private final float width;
    private final float height;

    public SvgMesh(List<? extends Triangle> triangles) {
        this(1.0f, 1.0f, triangles);
    }

    public SvgMesh(float width, float height, List<? extends Triangle> triangles) {
        if (!Float.isFinite(width) || !Float.isFinite(height) || width <= 0.0f || height <= 0.0f) {
            throw new IllegalArgumentException("SVG mesh 画布必须是有限正数");
        }
        this.width = width;
        this.height = height;
        this.triangles = List.copyOf(Objects.requireNonNull(triangles, "triangles"));
    }

    public float width() { return width; }

    public float height() { return height; }

    public List<Triangle> triangles() {
        return triangles;
    }

    public int triangleCount() {
        return triangles.size();
    }

    public int vertexCount() {
        return triangles.size() * 3;
    }

    public record Vertex(float x, float y, int color) {
        public Vertex {
            if (!Float.isFinite(x) || !Float.isFinite(y)) {
                throw new IllegalArgumentException("SVG mesh 顶点坐标必须是有限数");
            }
        }
    }

    public record Triangle(Vertex a, Vertex b, Vertex c) {
        public Triangle {
            Objects.requireNonNull(a, "triangle.a");
            Objects.requireNonNull(b, "triangle.b");
            Objects.requireNonNull(c, "triangle.c");
        }
    }
}
