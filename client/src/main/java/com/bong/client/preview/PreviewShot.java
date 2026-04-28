package com.bong.client.preview;

/**
 * 单角度截图配置 — name 决定输出文件 `preview-{name}.png`，tp/yaw/pitch 是
 * 拍摄前 client 端 setPos / setYaw / setPitch 的目标。
 *
 * 注：multi-player 下 server 不会主动同步 client.player 到这个位置；client 端
 * setPos 后会有几个 tick 的"漂移"窗口，足够 ScreenshotRecorder 抢一帧拍下。
 * 实际效果取决于 server view-distance 是否覆盖目标 chunk —— 若目标超出 server
 * 已发的 chunk 范围，会拍到空白。P0 选 spawn 周围安全；P1 多角度需配合 server
 * preview 模式做 server-side teleport（见 plan §2.4）。
 */
public record PreviewShot(
        String name,
        double[] tp,
        float yaw,
        float pitch
) {
    public PreviewShot {
        if (name == null || name.isBlank()) {
            throw new IllegalArgumentException("PreviewShot.name must be non-empty");
        }
        if (tp == null || tp.length != 3) {
            throw new IllegalArgumentException("PreviewShot.tp must be [x, y, z]");
        }
    }
}
