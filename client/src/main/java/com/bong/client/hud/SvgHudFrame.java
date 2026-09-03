package com.bong.client.hud;

/**
 * SVG HUD 的不可变语义 frame。
 *
 * <p>应用层在这里决定 layer 是否可见以及如何布局；表现后端只消费已经
 * 归一化的坐标、缩放和颜色，不反向读取业务 Store。</p>
 */
public record SvgHudFrame(boolean visible, int x, int y, float scale, int tint) {
    public SvgHudFrame {
        if (!Float.isFinite(scale) || scale <= 0.0f) {
            throw new IllegalArgumentException("SVG HUD scale 必须是有限正数");
        }
    }

    public static SvgHudFrame hidden() {
        return new SvgHudFrame(false, 0, 0, 1.0f, 0x00000000);
    }
}
