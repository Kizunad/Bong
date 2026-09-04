package com.bong.client.preview;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 锁住截图状态机与进程环境的分层边界。 */
class PreviewSessionConfigurationBoundaryTest {
    @Test
    void harnessReadsTheEnvironmentAndInjectsSvgPreviewFlag() throws IOException {
        String session = Files.readString(Path.of(
            "src/main/java/com/bong/client/preview/PreviewSession.java"
        ));
        String harness = Files.readString(Path.of(
            "src/main/java/com/bong/client/preview/PreviewHarnessClient.java"
        ));

        assertFalse(session.contains("System.getenv"),
            "PreviewSession 是状态机，不得直接读取进程环境");
        assertTrue(session.contains("PreviewSession(PreviewConfig config, boolean svgHudPreview)"),
            "SVG HUD fixture 开关必须通过构造参数注入状态机");
        assertTrue(harness.contains("new PreviewSession(config, \"1\".equals(System.getenv(ENV_SVG_HUD_PREVIEW)))"),
            "环境变量只能由 preview harness 读取后注入状态机");
    }
}
