package com.bong.client.ui.preview;

import java.util.Objects;

/** 单张真实 Fabric UI 截图的窗口、缩放和预期布局。 */
public record UiPreviewShot(
    String name,
    String sceneId,
    int framebufferWidth,
    int framebufferHeight,
    int guiScale,
    int expectedLogicalWidth,
    int expectedLogicalHeight,
    String expectedTemplateId
) {
    public UiPreviewShot {
        name = requireToken(name, "name");
        sceneId = requireToken(sceneId, "scene_id");
        expectedTemplateId = requireToken(expectedTemplateId, "expected_template_id");
        if (framebufferWidth <= 0 || framebufferHeight <= 0) {
            throw new IllegalArgumentException("framebuffer 尺寸必须为正数");
        }
        if (guiScale <= 0) {
            throw new IllegalArgumentException("gui_scale 必须为正数");
        }
        if (expectedLogicalWidth <= 0 || expectedLogicalHeight <= 0) {
            throw new IllegalArgumentException("预期逻辑 viewport 尺寸必须为正数");
        }
        int calculatedWidth = ceilDiv(framebufferWidth, guiScale);
        int calculatedHeight = ceilDiv(framebufferHeight, guiScale);
        if (expectedLogicalWidth != calculatedWidth || expectedLogicalHeight != calculatedHeight) {
            throw new IllegalArgumentException(String.format(
                "预期逻辑 viewport 与 framebuffer/gui_scale 不一致: expected=%dx%d, calculated=%dx%d",
                expectedLogicalWidth, expectedLogicalHeight, calculatedWidth, calculatedHeight));
        }
    }

    private static String requireToken(String value, String field) {
        Objects.requireNonNull(value, field + " must not be null");
        String normalized = value.strip();
        if (!normalized.matches("[a-z0-9][a-z0-9_-]*")) {
            throw new IllegalArgumentException(field + " 必须是安全的小写标识符: " + value);
        }
        return normalized;
    }

    private static int ceilDiv(int value, int divisor) {
        return value / divisor + (value % divisor == 0 ? 0 : 1);
    }
}
