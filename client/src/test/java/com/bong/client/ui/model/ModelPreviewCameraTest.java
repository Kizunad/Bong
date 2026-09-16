package com.bong.client.ui.model;

import net.minecraft.util.math.Box;
import org.joml.Vector3f;
import org.joml.Quaternionf;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class ModelPreviewCameraTest {
    @Test void fullGeometryFitsAtAnyRotationAndWindowAspect() {
        var camera = new ModelPreviewCamera();
        for (Box bounds : new Box[]{new Box(-.3, 0, -.3, .3, 1.8, .3),
            new Box(-18, -2, -35, 22, 6, 45), new Box(4, 8, 2, 4.05, 8.4, 2.1)}) {
            var center = bounds.getCenter();
            for (int[] viewport : new int[][]{{160, 100}, {450, 320}, {200, 420}}) {
                float scale = camera.scale(bounds, viewport[0], viewport[1]);
                for (int yaw : new int[]{0, 50, 120, 210}) for (int pitch : new int[]{-85, -25, 50, 85}) {
                    var rotation = new Quaternionf().rotationXYZ((float) Math.toRadians(pitch), (float) Math.toRadians(yaw), 0);
                    for (double x : new double[]{bounds.minX, bounds.maxX})
                        for (double y : new double[]{bounds.minY, bounds.maxY})
                            for (double z : new double[]{bounds.minZ, bounds.maxZ}) {
                                var point = rotation.transform(new Vector3f((float) (x - center.x),
                                    (float) (y - center.y), (float) (z - center.z))).mul(scale);
                                assertTrue(Math.abs(point.x) < viewport[0] / 2f && Math.abs(point.y) < viewport[1] / 2f,
                                    "细长/巨大/偏心模型的任何旋转均不得被初始取景裁切");
                            }
                }
            }
        }
    }

    @Test void focusArrivesAndStopsButManualDragCanInterrupt() {
        var camera = new ModelPreviewCamera();
        var target = new net.minecraft.util.math.Vec3d(.7,.8,.5);
        camera.focus(target, 2.4f, 8,-5,true);
        camera.advance(.4f);
        assertTrue(camera.moving());
        assertTrue(camera.zoom()<2.4f);
        camera.advance(2);
        assertFalse(camera.moving());
        assertEquals(target, camera.center(new Box(0,0,0,1,1,1)));
        float yaw=camera.yaw();
        camera.advance(5);
        assertEquals(yaw,camera.yaw(),"停在焦点后不能自动旋转");
        camera.focus(new net.minecraft.util.math.Vec3d(.2,.3,.4),2,180,-5,false);
        camera.advance(.2f);
        camera.drag(-10,0);
        yaw=camera.yaw();
        camera.advance(2);
        assertEquals(yaw,camera.yaw(),"手动拖拽后不能被旧镜头动画拉回");
    }

    @Test void stoppedOrbitOnlyRespondsToDragAndResetRestoresFit() {
        var camera = new ModelPreviewCamera();
        camera.autoRotate(false);
        float yaw = camera.yaw();
        camera.advance(1);
        assertEquals(yaw, camera.yaw(), "停止旋转必须真正停止自动运动");
        camera.drag(20, -10);
        assertTrue(camera.yaw() > yaw, "拖拽方向必须控制旋转方向");
        float dragged = camera.yaw();
        camera.advance(1);
        assertEquals(dragged, camera.yaw(), "释放拖拽后不能继续自行旋转");
        camera.scroll(3);
        assertTrue(camera.zoom() > 1);
        camera.reset();
        assertEquals(1, camera.zoom());
        assertFalse(camera.autoRotate(), "重新取景不得强制恢复自动旋转");
    }
}
