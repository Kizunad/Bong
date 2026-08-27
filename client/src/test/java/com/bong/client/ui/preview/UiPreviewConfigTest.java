package com.bong.client.ui.preview;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UiPreviewConfigTest {
    @Test
    void checkedInConfigCoversMinimumOddAndWideRealRendererCases() throws IOException {
        UiPreviewConfig config = UiPreviewConfig.parse(
            Files.readString(Path.of("ui-preview-harness.json")));
        assertEquals(3, config.screenshots().size());
        assertEquals("craft-compact", config.screenshots().get(0).expectedTemplateId());
        assertEquals(320, config.screenshots().get(0).expectedLogicalWidth());
        assertEquals(401, config.screenshots().get(1).expectedLogicalWidth(),
            "奇数 framebuffer 必须按 Minecraft 规则向上取整");
        assertEquals("craft", config.screenshots().get(2).expectedTemplateId());
        assertTrue(config.screenshots().stream().allMatch(shot -> UiPreviewScenes.isRegistered(shot.sceneId())));
    }

    @Test
    void defaultsAreAppliedToMinimalValidConfig(@TempDir Path tempDir) throws IOException {
        UiPreviewConfig config = UiPreviewConfig.parse(
            Files.readString(write(tempDir, validShot("craft"))));
        assertEquals("ui-preview-screenshots", config.outputDir());
        assertEquals(600, config.waitClientTicks());
        assertEquals(200, config.resizeTimeoutTicks());
        assertEquals(20, config.settleTicks());
        assertTrue(config.exitOnComplete());
    }

    @Test
    void explicitRuntimeOptionsArePreserved(@TempDir Path tempDir) throws IOException {
        String json = "{\n"
            + "  \"output_dir\": \"shots\",\n"
            + "  \"wait_client_ticks\": 40,\n"
            + "  \"resize_timeout_ticks\": 30,\n"
            + "  \"settle_ticks\": 5,\n"
            + "  \"exit_on_complete\": false,\n"
            + "  \"screenshots\": [" + shotBody("craft") + "]\n}";
        UiPreviewConfig config = UiPreviewConfig.parse(Files.readString(write(tempDir, json)));
        assertEquals("shots", config.outputDir());
        assertEquals(40, config.waitClientTicks());
        assertEquals(30, config.resizeTimeoutTicks());
        assertEquals(5, config.settleTicks());
        assertFalse(config.exitOnComplete());
    }

    @Test
    void malformedDimensionsScaleAndExpectedViewportAreRejected() {
        assertThrows(IllegalArgumentException.class, () ->
            new UiPreviewShot("x", "craft", 0, 480, 2, 320, 240, "craft-compact"));
        assertThrows(IllegalArgumentException.class, () ->
            new UiPreviewShot("x", "craft", 640, 480, 0, 320, 240, "craft-compact"));
        assertThrows(IllegalArgumentException.class, () ->
            new UiPreviewShot("x", "craft", 801, 481, 2, 400, 240, "craft-compact"));
    }

    @Test
    void unsafeSceneAndFileTokensAreRejected() {
        assertThrows(IllegalArgumentException.class, () ->
            new UiPreviewShot("../escape", "craft", 640, 480, 2, 320, 240, "craft-compact"));
        assertThrows(IllegalArgumentException.class, () ->
            new UiPreviewShot("safe", "java.lang.Screen", 640, 480, 2, 320, 240, "craft-compact"));
    }

    @Test
    void missingOrEmptyScreenshotsAreRejected(@TempDir Path tempDir) throws IOException {
        assertThrows(IllegalArgumentException.class, () ->
            UiPreviewConfig.parse(Files.readString(write(tempDir, "{}"))));
        assertThrows(IllegalArgumentException.class, () ->
            UiPreviewConfig.parse(Files.readString(write(tempDir, "{\"screenshots\":[]}"))));
    }

    @Test
    void duplicateShotNamesAreRejectedBeforeTheyCanOverwriteArtifacts() {
        UiPreviewShot first = shot("same-name", "craft", 640, 480);
        UiPreviewShot duplicate = shot("same-name", "craft", 800, 480);

        IllegalArgumentException failure = assertThrows(IllegalArgumentException.class, () ->
            new UiPreviewConfig("shots", 20, 20, 1, false, java.util.List.of(first, duplicate)));

        assertTrue(failure.getMessage().contains("name 必须唯一"));
    }

    private static String validShot(String sceneId) {
        return "{\"screenshots\":[" + shotBody(sceneId) + "]}";
    }

    private static String shotBody(String sceneId) {
        return "{\"name\":\"minimum\",\"scene_id\":\"" + sceneId
            + "\",\"framebuffer_width\":640,\"framebuffer_height\":480,\"gui_scale\":2,"
            + "\"expected_logical_width\":320,\"expected_logical_height\":240,"
            + "\"expected_template_id\":\"craft-compact\"}";
    }

    private static UiPreviewShot shot(
        String name,
        String sceneId,
        int framebufferWidth,
        int framebufferHeight
    ) {
        return new UiPreviewShot(
            name,
            sceneId,
            framebufferWidth,
            framebufferHeight,
            2,
            (framebufferWidth + 1) / 2,
            (framebufferHeight + 1) / 2,
            "craft-compact"
        );
    }

    private static Path write(Path directory, String body) throws IOException {
        Path path = directory.resolve("ui-preview.json");
        Files.writeString(path, body);
        return path;
    }
}
