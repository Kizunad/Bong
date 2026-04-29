package com.bong.client.preview;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * PreviewConfig chunk-ready 字段（plan-worldgen-snapshot-v1 §4.2）。
 *
 * 覆盖：
 *  - 新字段默认值（chunk_ready_radius=4 / chunk_ready_timeout_ticks=600）
 *  - JSON 显式覆盖
 *  - 负值校验
 *  - radius=0 退回旧行为路径
 */
class PreviewConfigChunkReadyTest {

    private static final String SHOTS_FRAGMENT =
            "  \"screenshots\": [\n"
                    + "    {\"name\": \"top\", \"tp\": [0, 100, 0], \"yaw\": 0, \"pitch\": 90}\n"
                    + "  ]\n";

    private Path writeJson(@TempDir Path tmp, String body) throws IOException {
        Path f = tmp.resolve("harness.json");
        Files.writeString(f, body);
        return f;
    }

    @Test
    void defaultsAppliedWhenFieldsAbsent(@TempDir Path tmp) throws IOException {
        // 不写 chunk_ready_* 字段
        String body = "{\n" + SHOTS_FRAGMENT + "}";
        PreviewConfig cfg = PreviewConfig.load(writeJson(tmp, body));
        assertEquals(4, cfg.chunkReadyRadius(),
                "chunk_ready_radius 缺省值应为 4 chunks（=9x9=81 chunks 覆盖）");
        assertEquals(600, cfg.chunkReadyTimeoutTicks(),
                "chunk_ready_timeout_ticks 缺省应为 600 ticks (30s)");
    }

    @Test
    void explicitOverrides(@TempDir Path tmp) throws IOException {
        String body = "{\n"
                + "  \"chunk_ready_radius\": 8,\n"
                + "  \"chunk_ready_timeout_ticks\": 1200,\n"
                + SHOTS_FRAGMENT
                + "}";
        PreviewConfig cfg = PreviewConfig.load(writeJson(tmp, body));
        assertEquals(8, cfg.chunkReadyRadius());
        assertEquals(1200, cfg.chunkReadyTimeoutTicks());
    }

    @Test
    void radiusZeroDisablesBarrier(@TempDir Path tmp) throws IOException {
        // chunk_ready_radius=0 = 退回 settle_ticks 盲等行为
        String body = "{\n"
                + "  \"chunk_ready_radius\": 0,\n"
                + SHOTS_FRAGMENT
                + "}";
        PreviewConfig cfg = PreviewConfig.load(writeJson(tmp, body));
        assertEquals(0, cfg.chunkReadyRadius());
    }

    @Test
    void negativeRadiusRejected(@TempDir Path tmp) {
        String body = "{\n"
                + "  \"chunk_ready_radius\": -1,\n"
                + SHOTS_FRAGMENT
                + "}";
        assertThrows(IllegalArgumentException.class,
                () -> PreviewConfig.load(writeJson(tmp, body)));
    }

    @Test
    void negativeTimeoutRejected(@TempDir Path tmp) {
        String body = "{\n"
                + "  \"chunk_ready_timeout_ticks\": -100,\n"
                + SHOTS_FRAGMENT
                + "}";
        assertThrows(IllegalArgumentException.class,
                () -> PreviewConfig.load(writeJson(tmp, body)));
    }

    @Test
    void allOtherFieldsRetainDefaults(@TempDir Path tmp) throws IOException {
        // 防回归：新增 chunk_ready_* 字段不应影响其他默认值
        String body = "{\n" + SHOTS_FRAGMENT + "}";
        PreviewConfig cfg = PreviewConfig.load(writeJson(tmp, body));
        assertEquals(1200, cfg.waitWorldTicks());
        assertEquals(100, cfg.waitChunksTicks());
        assertEquals(20, cfg.settleTicks());
        assertEquals("PreviewBot", cfg.username());
        assertEquals(1, cfg.screenshots().size());
    }
}
