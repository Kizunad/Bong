package com.bong.client.fauna;

/**
 * fauna 朝向平滑的纯数学（无 Minecraft 依赖，可纯 JVM 单测）。
 *
 * <p>抽离原因：这两个函数原在 {@link FaunaEntity}，但 {@code FaunaEntity extends Entity}——
 * 静态调用会强制加载 MC {@code Entity} 类，纯 JUnit（无 Bootstrap）下 {@code ExceptionInInitializerError}。
 * 移到独立无 MC 依赖类后可直测（与"mixin 包 helper 要抽独立类"同源的坑）。
 */
public final class FaunaYawMath {
    private FaunaYawMath() {}

    /**
     * 把 {@code current} 朝 {@code target} 转，单 tick 最多 {@code maxDelta} 度，处理 ±180 环绕。
     */
    public static float approachYaw(float current, float target, float maxDelta) {
        float diff = wrapDegrees(target - current);
        if (diff > maxDelta) {
            diff = maxDelta;
        } else if (diff < -maxDelta) {
            diff = -maxDelta;
        }
        return wrapDegrees(current + diff);
    }

    /** 归一到 (-180, 180]。 */
    public static float wrapDegrees(float deg) {
        float d = deg % 360.0f;
        if (d > 180.0f) {
            d -= 360.0f;
        } else if (d <= -180.0f) {
            d += 360.0f;
        }
        return d;
    }
}
