package com.bong.client.ui.preview;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UiPreviewResultFileTest {
    @Test
    void beginRemovesStaleResult(@TempDir Path outputDir) throws IOException {
        Path result = outputDir.resolve(UiPreviewResultFile.FILE_NAME);
        Files.writeString(result, "status=passed\n");

        new UiPreviewResultFile(outputDir.toString()).begin();

        assertFalse(Files.exists(result), "新一轮截图开始前必须移除旧结果，禁止历史成功假绿");
    }

    @Test
    void passedRecordsCompletedCount(@TempDir Path outputDir) throws IOException {
        new UiPreviewResultFile(outputDir.toString()).passed(7);

        String result = Files.readString(outputDir.resolve(UiPreviewResultFile.FILE_NAME));
        assertTrue(result.contains("status=passed\n"));
        assertTrue(result.contains("completed=7\n"));
    }

    @Test
    void failedRecordsContextOnSingleLines(@TempDir Path outputDir) throws IOException {
        new UiPreviewResultFile(outputDir.toString()).failed(
            5, "WAIT_VIEWPORT", "scale-three",
            new IllegalStateException("expected 334x241\nbut was 500x361")
        );

        String result = Files.readString(outputDir.resolve(UiPreviewResultFile.FILE_NAME));
        assertTrue(result.contains("status=failed\n"));
        assertTrue(result.contains("completed=5\n"));
        assertTrue(result.contains("phase=WAIT_VIEWPORT\n"));
        assertTrue(result.contains("shot=scale-three\n"));
        assertTrue(result.contains("expected 334x241 but was 500x361\n"));
    }
}
