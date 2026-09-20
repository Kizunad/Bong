package com.bong.client.ui.model;

import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.MeridianChannel;
import net.minecraft.client.model.ModelPart;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.math.Box;
import net.minecraft.util.math.Vec3d;
import org.joml.Matrix4f;
import org.joml.Vector3f;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/** 本帧真实玩家模型的内观定位；只定义显示锚点，不定义经脉物理或种族拓扑。 */
public final class BodyModelGeometry {
    private final Map<BodyPart, Region> parts = new EnumMap<>(BodyPart.class);
    private final Map<MeridianChannel, List<Vec3d>> paths = new EnumMap<>(MeridianChannel.class);
    private final Map<MeridianChannel, List<Vec3d>> returns = new EnumMap<>(MeridianChannel.class);
    private final Vec3d pool;

    public BodyModelGeometry(PlayerEntityModel<?> model, MatrixStack matrices) {
        this(model, part -> region(part, matrices));
    }

    BodyModelGeometry(PlayerEntityModel<?> model, java.util.function.Function<ModelPart, Region> regions) {
        var head = regions.apply(model.head);
        var body = regions.apply(model.body);
        parts.put(BodyPart.HEAD, head.slice(.20, 1));
        // 原版没有独立颈骨，在头部底段标注颈部，不额外造一块悬空几何。
        parts.put(BodyPart.NECK, head.slice(0, .20));
        parts.put(BodyPart.CHEST, body.slice(.40, 1));
        parts.put(BodyPart.ABDOMEN, body.slice(0, .40));
        limb(BodyPart.LEFT_UPPER_ARM, BodyPart.LEFT_FOREARM, BodyPart.LEFT_HAND, regions.apply(model.leftArm));
        limb(BodyPart.RIGHT_UPPER_ARM, BodyPart.RIGHT_FOREARM, BodyPart.RIGHT_HAND, regions.apply(model.rightArm));
        limb(BodyPart.LEFT_THIGH, BodyPart.LEFT_CALF, BodyPart.LEFT_FOOT, regions.apply(model.leftLeg));
        limb(BodyPart.RIGHT_THIGH, BodyPart.RIGHT_CALF, BodyPart.RIGHT_FOOT, regions.apply(model.rightLeg));
        pool = anchor(BodyPart.ABDOMEN, .5, .65, .5);
        for (var channel : MeridianChannel.values()) {
            var path = smooth(path(channel, 0));
            paths.put(channel, path);
            // 两条路径都锚定在同一组部件内部；末梢与池相接，不在世界坐标侧移。
            var back = new ArrayList<>(smooth(path(channel, .12)));
            back.set(0, path.get(0));
            back.set(back.size() - 1, path.get(path.size() - 1));
            java.util.Collections.reverse(back);
            returns.put(channel, List.copyOf(back));
        }
    }

    private void limb(BodyPart top, BodyPart mid, BodyPart end, Region region) {
        parts.put(top, region.slice(.55, 1));
        parts.put(mid, region.slice(.18, .55));
        parts.put(end, region.slice(0, .18));
    }

    static Region region(ModelPart part, MatrixStack matrices) {
        var regions = new ArrayList<Region>(1);
        part.forEachCuboid(matrices, (entry, path, index, cube) -> regions.add(new Region(
            new Box(cube.minX / 16.0, cube.minY / 16.0, cube.minZ / 16.0,
                cube.maxX / 16.0, cube.maxY / 16.0, cube.maxZ / 16.0),
            new Matrix4f(entry.getPositionMatrix()), BodyModelDeformation.capture(cube))));
        if (regions.size() != 1) throw new IllegalStateException("玩家内观部件需要一个原版主体立方体");
        return regions.get(0);
    }

    public Region part(BodyPart part) { return parts.get(part); }
    public Vec3d pool() { return pool; }
    public List<Vec3d> route(MeridianChannel channel) { return paths.get(channel); }
    public List<Vec3d> returnRoute(MeridianChannel channel) { return returns.get(channel); }

    /** 保留部件局部体积和最终变换；旋转后的命中仍在实际部件内，而非外包 AABB。 */
    public record Region(Box local, Matrix4f transform, BodyModelDeformation deformation) {
        Region slice(double bottom, double top) {
            return new Region(new Box(local.minX, local.maxY - local.getYLength() * top, local.minZ,
                local.maxX, local.maxY - local.getYLength() * bottom, local.maxZ), transform, deformation);
        }
        public Vec3d anchor(double x, double y, double front) {
            return transform(new Vec3d(local.minX + local.getXLength() * x,
                local.maxY - local.getYLength() * y, local.maxZ - local.getZLength() * front));
        }
        private Vec3d transform(Vec3d point) {
            return BodyModelGeometry.transform(transform, deformation == null ? point : deformation.apply(point));
        }
        public Vec3d center() { return transform(local.getCenter()); }
        public List<Vec3d> corners() {
            var points = new ArrayList<Vec3d>(8);
            for (double x : new double[]{local.minX, local.maxX})
                for (double y : new double[]{local.minY, local.maxY})
                    for (double z : new double[]{local.minZ, local.maxZ}) points.add(transform(new Vec3d(x, y, z)));
            return points;
        }
        /** 按原模型的像素网格细分，弯曲的边框和命中面不能仍使用直立立方体。 */
        public List<List<Vec3d>> edges() {
            var result = new ArrayList<List<Vec3d>>(12);
            var points = new ArrayList<Vec3d>(8);
            for (double x : new double[]{local.minX, local.maxX})
                for (double y : new double[]{local.minY, local.maxY})
                    for (double z : new double[]{local.minZ, local.maxZ}) points.add(new Vec3d(x, y, z));
            for (int i = 0; i < 8; i++) for (int axis : new int[]{1, 2, 4}) {
                int j = i ^ axis;
                if (j <= i) continue;
                Vec3d a = points.get(i), b = points.get(j);
                int steps = deformation == null ? 1 : Math.max(1, (int) Math.ceil(a.distanceTo(b) * 16));
                var edge = new ArrayList<Vec3d>(steps + 1);
                for (int step = 0; step <= steps; step++) edge.add(transform(a.lerp(b, step / (double) steps)));
                result.add(edge);
            }
            return result;
        }
        public Box bounds() { return BodyModelGeometry.bounds(deformation == null ? corners() : surface()); }
        public java.util.Optional<Vec3d> raycast(Vec3d start, Vec3d end) {
            if (deformation != null) {
                var vertices = surface();
                Vec3d direction = end.subtract(start);
                double nearest = Double.POSITIVE_INFINITY;
                for (int i = 0; i < vertices.size(); i += 4) {
                    var a = vertices.get(i); var b = vertices.get(i + 1);
                    var c = vertices.get(i + 2); var d = vertices.get(i + 3);
                    nearest = Math.min(nearest, triangle(start, direction, a, b, c));
                    nearest = Math.min(nearest, triangle(start, direction, a, c, d));
                }
                return Double.isFinite(nearest) ? java.util.Optional.of(start.add(direction.multiply(nearest)))
                    : java.util.Optional.empty();
            }
            var inverse = new Matrix4f(transform).invert();
            return local.raycast(BodyModelGeometry.transform(inverse, start), BodyModelGeometry.transform(inverse, end))
                .map(this::transform);
        }

        private List<Vec3d> surface() {
            var vertices = new ArrayList<Vec3d>();
            double[] min = {local.minX, local.minY, local.minZ}, max = {local.maxX, local.maxY, local.maxZ};
            for (int normal = 0; normal < 3; normal++) {
                int u = (normal + 1) % 3, v = (normal + 2) % 3;
                int us = Math.max(1, (int) Math.ceil((max[u] - min[u]) * 16));
                int vs = Math.max(1, (int) Math.ceil((max[v] - min[v]) * 16));
                for (double face : new double[]{min[normal], max[normal]})
                    for (int i = 0; i < us; i++) for (int j = 0; j < vs; j++) {
                        for (int[] corner : new int[][]{{0, 0}, {1, 0}, {1, 1}, {0, 1}}) {
                            double[] p = new double[3];
                            p[normal] = face;
                            p[u] = min[u] + (max[u] - min[u]) * (i + corner[0]) / us;
                            p[v] = min[v] + (max[v] - min[v]) * (j + corner[1]) / vs;
                            vertices.add(transform(new Vec3d(p[0], p[1], p[2])));
                        }
                    }
            }
            return vertices;
        }
        private static double triangle(Vec3d start, Vec3d direction, Vec3d a, Vec3d b, Vec3d c) {
            Vec3d ab = b.subtract(a), ac = c.subtract(a), p = direction.crossProduct(ac);
            double determinant = ab.dotProduct(p);
            if (Math.abs(determinant) < 1e-9) return Double.POSITIVE_INFINITY;
            Vec3d t = start.subtract(a);
            double u = t.dotProduct(p) / determinant;
            if (u < 0 || u > 1) return Double.POSITIVE_INFINITY;
            Vec3d q = t.crossProduct(ab);
            double v = direction.dotProduct(q) / determinant;
            if (v < 0 || u + v > 1) return Double.POSITIVE_INFINITY;
            double distance = ac.dotProduct(q) / determinant;
            return distance >= 0 && distance <= 1 ? distance : Double.POSITIVE_INFINITY;
        }
    }

    private Vec3d anchor(BodyPart part, double x, double y, double front) { return parts.get(part).anchor(x, y, front); }
    private static Vec3d transform(Matrix4f matrix, Vec3d point) {
        var v = matrix.transformPosition((float) point.x, (float) point.y, (float) point.z, new Vector3f());
        return new Vec3d(v.x, v.y, v.z);
    }
    public static Box bounds(List<Vec3d> path) {
        Box result = new Box(path.get(0), path.get(0));
        for (var point : path) result = result.union(new Box(point, point));
        return result;
    }
    public static Vec3d normalized(Vec3d point, Box bounds) {
        return new Vec3d((point.x - bounds.minX) / bounds.getXLength(),
            (point.y - bounds.minY) / bounds.getYLength(), (point.z - bounds.minZ) / bounds.getZLength());
    }

    private static List<Vec3d> smooth(List<Vec3d> points) {
        // Chaikin 切角保留首尾；避免流路折成机械直角。
        for (int round = 0; round < 2; round++) {
            var next = new ArrayList<Vec3d>();
            next.add(points.get(0));
            for (int i = 1; i < points.size(); i++) {
                next.add(points.get(i - 1).lerp(points.get(i), .25));
                next.add(points.get(i - 1).lerp(points.get(i), .75));
            }
            next.add(points.get(points.size() - 1));
            points = next;
        }
        return List.copyOf(points);
    }

    private List<Vec3d> path(MeridianChannel channel, double back) {
        double lane = switch (channel) {
            case LU, LI, SP, ST -> -.16;
            case HT, SI, KI, BL -> 0;
            default -> .16;
        };
        boolean left = channel.region() == MeridianChannel.BodyRegion.LEFT_ARM
            || channel.region() == MeridianChannel.BodyRegion.LEFT_LEG;
        double side = left ? 1 : -1;
        double depth = .60 - back;
        return switch (channel.region()) {
            case LEFT_ARM, RIGHT_ARM -> {
                var upper = left ? BodyPart.LEFT_UPPER_ARM : BodyPart.RIGHT_UPPER_ARM;
                var forearm = left ? BodyPart.LEFT_FOREARM : BodyPart.RIGHT_FOREARM;
                var hand = left ? BodyPart.LEFT_HAND : BodyPart.RIGHT_HAND;
                yield List.of(pool, anchor(BodyPart.CHEST, .5 + side * .18, .3, depth),
                    anchor(BodyPart.CHEST, .5 + side * .4, .82, depth),
                    anchor(upper, .5 + lane, .75, depth), anchor(forearm, .5 + lane, .7, depth),
                    anchor(hand, .5 + lane, .4, depth));
            }
            case LEFT_LEG, RIGHT_LEG -> {
                var thigh = left ? BodyPart.LEFT_THIGH : BodyPart.RIGHT_THIGH;
                var calf = left ? BodyPart.LEFT_CALF : BodyPart.RIGHT_CALF;
                var foot = left ? BodyPart.LEFT_FOOT : BodyPart.RIGHT_FOOT;
                yield List.of(pool, anchor(BodyPart.ABDOMEN, .5 + side * .2, .2, depth),
                    anchor(thigh, .5 + lane, .85, depth), anchor(calf, .5 + lane, .7, depth),
                    anchor(foot, .5 + lane, .35, depth));
            }
            default -> switch (channel) {
                case REN -> List.of(pool, anchor(BodyPart.CHEST, .5, .2, .75 - back),
                    anchor(BodyPart.NECK, .5, .5, .75 - back), anchor(BodyPart.HEAD, .5, .35, .7 - back));
                case DU -> List.of(pool, anchor(BodyPart.ABDOMEN, .5, .2, .25 + back),
                    anchor(BodyPart.CHEST, .5, .6, .25 + back), anchor(BodyPart.HEAD, .5, .7, .25 + back));
                case CHONG -> List.of(pool, anchor(BodyPart.CHEST, .63, .15, .45 - back),
                    anchor(BodyPart.CHEST, .63, .75, .45 - back), anchor(BodyPart.NECK, .6, .7, .45 - back));
                case DAI -> List.of(pool, anchor(BodyPart.ABDOMEN, .87, .65, depth),
                    anchor(BodyPart.ABDOMEN, .87, .65 - back, .2), anchor(BodyPart.ABDOMEN, .13, .65 - back, .2),
                    anchor(BodyPart.ABDOMEN, .13, .65, depth), pool);
                case YIN_QIAO, YANG_QIAO -> {
                    boolean yin = channel == MeridianChannel.YIN_QIAO;
                    var thigh = yin ? BodyPart.LEFT_THIGH : BodyPart.RIGHT_THIGH;
                    var foot = yin ? BodyPart.LEFT_FOOT : BodyPart.RIGHT_FOOT;
                    double x = yin ? .38 : .7;
                    yield List.of(pool, anchor(thigh, x, .5, depth), anchor(foot, x, .5, depth),
                        anchor(thigh, x, .8, .3 + back), anchor(BodyPart.CHEST, x, .75, .3 + back),
                        anchor(BodyPart.HEAD, x, .55, depth));
                }
                default -> {
                    double x = channel == MeridianChannel.YIN_WEI ? .8 : .2;
                    yield List.of(pool, anchor(BodyPart.ABDOMEN, x, .2, depth),
                        anchor(BodyPart.CHEST, x, .3, depth), anchor(BodyPart.CHEST, x, .87, depth),
                        anchor(BodyPart.NECK, .5 + (x - .5) * .4, .3, depth));
                }
            };
        };
    }

    /** 按实际路长推进；转弯处速度一致，终点折回与池连接。 */
    public static Vec3d sample(List<Vec3d> path, double progress) {
        double total = 0;
        for (int i = 1; i < path.size(); i++) total += path.get(i).distanceTo(path.get(i - 1));
        double distance = Math.max(0, Math.min(1, progress)) * total;
        for (int i = 1; i < path.size(); i++) {
            double segment = path.get(i).distanceTo(path.get(i - 1));
            if (distance <= segment && segment > 0) return path.get(i - 1).lerp(path.get(i), distance / segment);
            distance -= segment;
        }
        return path.get(path.size() - 1);
    }
}
