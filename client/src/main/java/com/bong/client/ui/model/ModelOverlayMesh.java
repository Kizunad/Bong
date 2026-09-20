package com.bong.client.ui.model;

import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.math.Vec3d;

/** 以实体局部坐标绘制细管与光点，与模型共用变换和裁剪。 */
public final class ModelOverlayMesh {
    private ModelOverlayMesh() {}
    public static void tube(VertexConsumer buffer, MatrixStack matrices, Vec3d a, Vec3d b, double radius, int color) {
        var tangent = b.subtract(a).normalize();
        if (tangent.lengthSquared() < .000001) return;
        var axis = Math.abs(tangent.y) > .9 ? new Vec3d(1, 0, 0) : new Vec3d(0, 1, 0);
        var u = tangent.crossProduct(axis).normalize().multiply(radius);
        var v = tangent.crossProduct(u).normalize().multiply(radius);
        for (int i = 0; i < 6; i++) {
            double angle = i * Math.PI / 3, next = (i + 1) * Math.PI / 3;
            var p = u.multiply(Math.cos(angle)).add(v.multiply(Math.sin(angle)));
            var q = u.multiply(Math.cos(next)).add(v.multiply(Math.sin(next)));
            vertex(buffer, matrices, a.add(p), color); vertex(buffer, matrices, b.add(p), color);
            vertex(buffer, matrices, b.add(q), color); vertex(buffer, matrices, a.add(q), color);
        }
    }
    public static void orb(VertexConsumer buffer, MatrixStack matrices, Vec3d center, double radius, int color) {
        for (int latitude = 0; latitude < 8; latitude++) for (int longitude = 0; longitude < 12; longitude++) {
            vertex(buffer, matrices, sphere(center, radius, latitude, longitude), color);
            vertex(buffer, matrices, sphere(center, radius, latitude + 1, longitude), color);
            vertex(buffer, matrices, sphere(center, radius, latitude + 1, longitude + 1), color);
            vertex(buffer, matrices, sphere(center, radius, latitude, longitude + 1), color);
        }
    }
    private static Vec3d sphere(Vec3d center, double radius, int latitude, int longitude) {
        double a = latitude * Math.PI / 8, b = longitude * Math.PI / 6;
        return center.add(radius * Math.sin(a) * Math.cos(b), radius * Math.cos(a), radius * Math.sin(a) * Math.sin(b));
    }
    private static void vertex(VertexConsumer b, MatrixStack matrices, Vec3d p, int color) {
        b.vertex(matrices.peek().getPositionMatrix(), (float) p.x, (float) p.y, (float) p.z)
            .color(color >> 16 & 255, color >> 8 & 255, color & 255, color >>> 24).next();
    }
}
