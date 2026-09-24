package com.bong.client.ui.model;

import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.MeridianChannel;
import net.minecraft.client.model.Dilation;
import net.minecraft.client.model.TexturedModelData;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.math.Box;
import net.minecraft.util.math.RotationAxis;
import net.minecraft.util.math.Vec3d;
import org.joml.Matrix4f;
import org.joml.Vector3f;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/** 用原版实际渲染顶点锁住错位回归，不能拿另一份坐标表与实现互相证明。 */
class BodyModelGeometryTest {
    @Test void animatedBendUsesTheSameMeshAsThePlayerAnimationLibrary() {
        var data = new io.github.kosmx.bendylib.ICuboidBuilder.Data(
            0, 0, -2, -2, -2, 4, 12, 4, 0, 0, 0, false, 64, 64);
        var builder = new io.github.kosmx.bendylib.impl.BendableCuboid.Builder();
        builder.direction = net.minecraft.util.math.Direction.DOWN;
        var bent = builder.build(data);
        bent.applyBend(.3f, .8f);
        var root = root(true);
        var region = new BodyModelGeometry.Region(new Box(-.125, -.125, -.125, .125, .625, .125),
            new Matrix4f(root.peek().getPositionMatrix()), new BodyModelDeformation(bent, bent.getBendAxis(), bent.getBend()));
        var mesh = new ModelPreviewMesh();
        bent.render(root.peek(), mesh.getBuffer(null), 1, 1, 1, 1, 0, 0);
        sameBounds(mesh.bounds(), region.bounds(), "弯曲动画的真实网格与内观定位必须一致");
        var rigid = new BodyModelGeometry.Region(region.local(), region.transform(), null);
        assertTrue(region.anchor(.5, .9, 1).distanceTo(rigid.anchor(.5, .9, 1)) > .01,
            "夹具必须实际发生肢体弯曲，不能只测试刚体旋转");
        var hit = region.raycast(region.anchor(.5, .9, 3), region.anchor(.5, .9, -3)).orElseThrow();
        assertTrue(hit.distanceTo(region.anchor(.5, .9, 1)) < 1e-5,
            "弯曲后点击仍要命中模型表面");
    }

    @Test void bodyRegionsFollowActualRenderedPartsForBothSkinsAndChangedPose() {
        for (boolean slim : new boolean[]{false, true}) for (boolean posed : new boolean[]{false, true}) {
            var model = model(slim);
            if (posed) {
                model.head.yaw = .6f;
                model.leftArm.pitch = -.8f;
                model.rightLeg.roll = .3f;
            }
            var root = root(posed);
            var geometry = new BodyModelGeometry(model, root);
            var actual = List.of(model.head, model.body, model.leftArm, model.rightArm, model.leftLeg, model.rightLeg);
            var groups = List.of(
                List.of(BodyPart.HEAD, BodyPart.NECK), List.of(BodyPart.CHEST, BodyPart.ABDOMEN),
                List.of(BodyPart.LEFT_UPPER_ARM, BodyPart.LEFT_FOREARM, BodyPart.LEFT_HAND),
                List.of(BodyPart.RIGHT_UPPER_ARM, BodyPart.RIGHT_FOREARM, BodyPart.RIGHT_HAND),
                List.of(BodyPart.LEFT_THIGH, BodyPart.LEFT_CALF, BodyPart.LEFT_FOOT),
                List.of(BodyPart.RIGHT_THIGH, BodyPart.RIGHT_CALF, BodyPart.RIGHT_FOOT));
            for (int i = 0; i < actual.size(); i++) {
                // 只显示一个真实部件，但仍走整个 PlayerEntityModel.render 的年龄/根变换链。
                model.setVisible(false);
                actual.get(i).visible = true;
                var mesh = new ModelPreviewMesh();
                model.render(root, mesh.getBuffer(null), 0, 0, 1, 1, 1, 1);
                Box overlay = geometry.part(groups.get(i).get(0)).bounds();
                for (var part : groups.get(i)) overlay = overlay.union(geometry.part(part).bounds());
                sameBounds(mesh.bounds(), overlay, "真实模型与部位边界错位：" + groups.get(i) + ", slim=" + slim);
            }
        }
    }

    @Test void circulationStaysInsideBodyAndEndsInTheCorrectLimb() {
        for (boolean slim : new boolean[]{false, true}) {
            var model = model(slim);
            var geometry = new BodyModelGeometry(model, root(false));
            for (var channel : MeridianChannel.values()) {
                var outward = geometry.route(channel);
                var back = geometry.returnRoute(channel);
                assertEquals(outward.get(0), back.get(back.size() - 1), "回流必须回到同一真元池");
                assertEquals(outward.get(outward.size() - 1), back.get(0), "末梢不能瞬移到另一条回路");
                for (var route : List.of(outward, back)) for (var point : route) {
                    assertTrue(java.util.Arrays.stream(BodyPart.values()).anyMatch(part -> contains(geometry.part(part), point)),
                        "经脉不得穿出真实身体：" + channel + ", point=" + point + ", slim=" + slim);
                }
            }
            assertTrue(contains(geometry.part(BodyPart.LEFT_HAND), BodyModelGeometry.sample(geometry.route(MeridianChannel.LU), 1)));
            assertTrue(contains(geometry.part(BodyPart.RIGHT_FOOT), BodyModelGeometry.sample(geometry.route(MeridianChannel.ST), 1)));
        }
    }

    @Test void rotatedPartPickingHitsItsActualSurface() {
        var model = model(true);
        model.leftArm.pitch = -.9f;
        var matrices = root(true);
        var geometry = new BodyModelGeometry(model, matrices);
        var region = geometry.part(BodyPart.LEFT_FOREARM);
        Vec3d near = region.anchor(.5, .5, 3), far = region.anchor(.5, .5, -3);
        var hit = region.raycast(near, far).orElseThrow();
        assertTrue(hit.distanceTo(region.anchor(.5, .5, 1)) < 1e-5,
            "旋转后的鼠标命中必须落在真实部件表面");
        assertTrue(region.raycast(region.anchor(1.1, .5, 3), region.anchor(1.1, .5, -3)).isEmpty(),
            "部件外包围盒内的空白区域不能误选中肢体");
    }

    private static PlayerEntityModel<AbstractClientPlayerEntity> model(boolean slim) {
        var model = new PlayerEntityModel<AbstractClientPlayerEntity>(TexturedModelData.of(
            PlayerEntityModel.getTexturedModelData(Dilation.NONE, slim), 64, 64).createModel(), slim);
        model.child = false; // 与真实玩家 renderer 同步后的状态一致。
        return model;
    }
    private static MatrixStack root(boolean posed) {
        var matrices = new MatrixStack();
        matrices.translate(.13, 1.5, -.2);
        matrices.scale(.9375f, -.9375f, -.9375f);
        if (posed) matrices.multiply(RotationAxis.POSITIVE_Y.rotationDegrees(37));
        return matrices;
    }
    private static boolean contains(BodyModelGeometry.Region region, Vec3d point) {
        var local = new Matrix4f(region.transform()).invert().transformPosition(
            (float) point.x, (float) point.y, (float) point.z, new Vector3f());
        return region.local().expand(1e-6).contains(local.x, local.y, local.z);
    }
    private static void sameBounds(Box expected, Box actual, String reason) {
        assertNotNull(expected, "真实模型必须产出顶点");
        assertEquals(expected.minX, actual.minX, 1e-6, reason);
        assertEquals(expected.minY, actual.minY, 1e-6, reason);
        assertEquals(expected.minZ, actual.minZ, 1e-6, reason);
        assertEquals(expected.maxX, actual.maxX, 1e-6, reason);
        assertEquals(expected.maxY, actual.maxY, 1e-6, reason);
        assertEquals(expected.maxZ, actual.maxZ, 1e-6, reason);
    }
}
