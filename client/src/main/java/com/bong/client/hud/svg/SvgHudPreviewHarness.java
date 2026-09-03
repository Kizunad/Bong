package com.bong.client.hud.svg;

/** 仅由显式环境变量激活的 SVG 截图 fixture，不改变正常联机状态。 */
public final class SvgHudPreviewHarness {
    private static final String ENV_ENABLED = "BONG_SVG_HUD_PREVIEW";

    private SvgHudPreviewHarness() {
    }

    public static void install() {
        if (!"1".equals(System.getenv(ENV_ENABLED))) {
            return;
        }
        // 预览开关只在显式 fixture 环境中打开，生产联机不会加载示例面板。
        SvgHudBackend.enablePreviewExample();
    }
}
