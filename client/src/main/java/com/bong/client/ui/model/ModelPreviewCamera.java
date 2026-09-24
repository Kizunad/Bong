package com.bong.client.ui.model;

import net.minecraft.util.math.Box;
import net.minecraft.util.math.Vec3d;

/** 正交取景使用真实几何的包围球；旋转任意角度仍能完整落在窗口内。 */
public final class ModelPreviewCamera {
    private float yaw = 25;
    private float pitch = -10;
    private float zoom = 1;
    private boolean autoRotate = true;
    private Vec3d focus = new Vec3d(.5, .5, .5);
    private Vec3d startFocus = focus, endFocus = focus;
    private float startYaw, endYaw, startPitch, endPitch, startZoom, endZoom;
    private float transitionTime, transitionDuration;

    public float yaw() { return yaw; }
    public float pitch() { return pitch; }
    public float zoom() { return zoom; }
    public boolean autoRotate() { return autoRotate; }
    public void autoRotate(boolean enabled) { autoRotate = enabled; }
    public void reset() { yaw = 25; pitch = -10; zoom = 1; focus = new Vec3d(.5, .5, .5); transitionDuration = 0; }
    public boolean moving() { return transitionTime < transitionDuration; }
    public Vec3d center(Box bounds) {
        return new Vec3d(bounds.minX + bounds.getXLength() * focus.x,
            bounds.minY + bounds.getYLength() * focus.y, bounds.minZ + bounds.getZLength() * focus.z);
    }
    /** 归一化焦点随模型尺寸变化；仅为镜头插值，不修改实体姿态或领域数据。 */
    public void focus(Vec3d target, float targetZoom, float targetYaw, float targetPitch, boolean entrance) {
        autoRotate = false;
        if (entrance) { focus = new Vec3d(.5, .5, .5); zoom = .62f; yaw = targetYaw - 135; pitch = 8; }
        startFocus = focus; endFocus = target;
        startYaw = yaw; endYaw = yaw + ((targetYaw - yaw + 180) % 360 + 360) % 360 - 180;
        startPitch = pitch; endPitch = targetPitch;
        startZoom = zoom; endZoom = targetZoom;
        transitionTime = 0; transitionDuration = entrance ? 1.35f : .65f;
    }
    public void settle() { if (moving()) advance(transitionDuration); }
    public void advance(float seconds) {
        if (moving()) {
            transitionTime = Math.min(transitionDuration, transitionTime + Math.max(0, seconds));
            float t = transitionTime / transitionDuration;
            float eased = t * t * t * (t * (t * 6 - 15) + 10);
            focus = startFocus.lerp(endFocus, eased);
            yaw = startYaw + (endYaw - startYaw) * eased;
            pitch = startPitch + (endPitch - startPitch) * eased;
            zoom = startZoom + (endZoom - startZoom) * eased;
        } else if (autoRotate) yaw = (yaw + seconds * 22) % 360;
    }
    public void drag(double dx, double dy) {
        if (autoRotate) return;
        transitionDuration = 0;
        yaw = (yaw + (float) dx * .75f) % 360;
        pitch = Math.max(-85, Math.min(85, pitch + (float) dy * .5f));
    }
    public void scroll(double amount) { transitionDuration = 0; zoom = Math.max(.25f, Math.min(4, zoom * (float) Math.pow(1.12, amount))); }

    public float scale(Box bounds, int width, int height) {
        double diameter = Math.sqrt(bounds.getXLength() * bounds.getXLength()
            + bounds.getYLength() * bounds.getYLength() + bounds.getZLength() * bounds.getZLength());
        return (float) (Math.max(1, Math.min(width, height) - 24) / Math.max(.001, diameter)) * zoom;
    }
}
