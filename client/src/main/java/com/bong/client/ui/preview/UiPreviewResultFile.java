package com.bong.client.ui.preview;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/** 在真实客户端进程与 Gradle 验收任务之间传递截图结果。 */
final class UiPreviewResultFile {
    static final String FILE_NAME = "ui-preview-result.txt";

    private UiPreviewResultFile() {
    }

    static void begin(Path outputDir) throws IOException {
        Files.createDirectories(outputDir);
        Files.deleteIfExists(outputDir.resolve(FILE_NAME));
    }

    static void passed(Path outputDir, int completed) throws IOException {
        write(outputDir, "status=passed\ncompleted=" + completed + "\n");
    }

    static void failed(
        Path outputDir,
        int completed,
        String phase,
        String shot,
        Throwable failure
    ) throws IOException {
        String detail = singleLine(failure == null ? "unknown failure" : failure.toString());
        write(outputDir, "status=failed\n"
            + "completed=" + completed + "\n"
            + "phase=" + singleLine(phase) + "\n"
            + "shot=" + singleLine(shot) + "\n"
            + "detail=" + detail + "\n");
    }

    private static void write(Path outputDir, String content) throws IOException {
        Files.writeString(outputDir.resolve(FILE_NAME), content);
    }

    private static String singleLine(String value) {
        return value.replace('\r', ' ').replace('\n', ' ');
    }
}
