package com.bong.client.ui.preview;

import net.minecraft.client.texture.NativeImage;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Objects;

/** 本地文件系统产物仓储；隔离截图状态机与 {@link Files} 基础设施。 */
final class UiPreviewResultFile implements UiPreviewArtifactSink {
    static final String FILE_NAME = "ui-preview-result.txt";
    private final Path outputDir;

    UiPreviewResultFile(String outputDir) {
        this.outputDir = Path.of(Objects.requireNonNull(outputDir, "outputDir 不能为空")).toAbsolutePath();
    }

    @Override
    public void begin() throws IOException {
        Files.createDirectories(outputDir);
        Files.deleteIfExists(outputDir.resolve(FILE_NAME));
    }

    @Override
    public String writeShot(String shotName, NativeImage image, String metadata) throws IOException {
        Path imagePath = outputDir.resolve("ui-" + shotName + ".png");
        image.writeTo(imagePath);
        Files.writeString(outputDir.resolve("ui-" + shotName + ".txt"), metadata);
        return imagePath.toString();
    }

    @Override
    public void passed(int completed) throws IOException {
        write("status=passed\ncompleted=" + completed + "\n");
    }

    @Override
    public void failed(
        int completed,
        String phase,
        String shot,
        Throwable failure
    ) throws IOException {
        String detail = singleLine(failure == null ? "unknown failure" : failure.toString());
        write("status=failed\n"
            + "completed=" + completed + "\n"
            + "phase=" + singleLine(phase) + "\n"
            + "shot=" + singleLine(shot) + "\n"
            + "detail=" + detail + "\n");
    }

    private void write(String content) throws IOException {
        Files.writeString(outputDir.resolve(FILE_NAME), content);
    }

    private static String singleLine(String value) {
        return value.replace('\r', ' ').replace('\n', ' ');
    }
}
