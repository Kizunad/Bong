package com.bong.client.preview;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PreviewSessionScreenCloseTest {
    @Test
    void tickLoopEmergencyCloseBypassesScreenTransitionLayer() throws IOException {
        String source = Files.readString(Path.of(
            "src/main/java/com/bong/client/preview/PreviewSession.java"
        ));

        assertTrue(
            source.contains("ScreenTransitionController.cancelAndClose(client)"),
            "preview harness 每 tick 紧急清屏必须绕过全局 setScreen 过渡层，"
                + "否则 GameMenuScreen 的关闭动画会被重复取消"
        );
        assertFalse(
            source.contains("client.setScreen(null)"),
            "preview harness 不能在 tick loop 里直接 client.setScreen(null)，"
                + "该路径会被 ScreenSetMixin 拦截成可反复重置的关闭动画"
        );
    }
}
