package com.bong.client.ui.preview;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UiPreviewSessionTest {
    @Test
    void cleanupFailureIsSuppressedWithoutReplacingPrimaryFailure() {
        RuntimeException primary = new RuntimeException("render failed");
        IllegalStateException cleanupFailure = new IllegalStateException("cleanup failed");

        Throwable actual = UiPreviewSession.attachCleanupFailure(primary, () -> {
            throw cleanupFailure;
        });

        assertSame(primary, actual, "cleanup 失败不能覆盖最初的截图失败");
        assertEquals(1, actual.getSuppressed().length, "cleanup 失败必须作为唯一 suppressed 原因保留");
        assertSame(cleanupFailure, actual.getSuppressed()[0]);
    }

    @Test
    void successfulCleanupLeavesPrimaryFailureUnchanged() {
        RuntimeException primary = new RuntimeException("render failed");

        Throwable actual = UiPreviewSession.attachCleanupFailure(primary, () -> {
        });

        assertSame(primary, actual);
        assertEquals(0, actual.getSuppressed().length, "成功 cleanup 不应制造伪 suppressed 原因");
    }

    @Test
    void screenCloseFailureDoesNotSkipSceneCleanup() {
        RuntimeException primary = new RuntimeException("render failed");
        IllegalStateException screenFailure = new IllegalStateException("screen close failed");
        IllegalArgumentException sceneFailure = new IllegalArgumentException("scene cleanup failed");
        List<String> calls = new ArrayList<>();

        Throwable actual = UiPreviewSession.attachCleanupFailures(
            primary,
            () -> {
                calls.add("screen");
                throw screenFailure;
            },
            () -> {
                calls.add("scene");
                throw sceneFailure;
            }
        );

        assertSame(primary, actual, "清理链失败不能覆盖最初的截图失败");
        assertEquals(List.of("screen", "scene"), calls,
            "Screen 关闭失败后仍必须继续清理 Scene fixture");
        assertEquals(List.of(screenFailure, sceneFailure), List.of(actual.getSuppressed()),
            "每个清理失败都必须按发生顺序保留为 suppressed 原因");
    }

    @Test
    void completionAlwaysStopsTicksButOnlyStopsClientWhenConfigured() {
        UiPreviewSession.CompletionDecision keepClient = UiPreviewSession.completionDecision(false);
        UiPreviewSession.CompletionDecision exitClient = UiPreviewSession.completionDecision(true);

        assertTrue(keepClient.stopTicks(), "保留客户端时也必须停止 tick 回调，避免重复写结果");
        assertFalse(keepClient.stopClient(), "exit_on_complete=false 不应停止客户端");
        assertTrue(exitClient.stopTicks(), "退出客户端前必须先终止 tick 状态机");
        assertTrue(exitClient.stopClient(), "exit_on_complete=true 必须请求停止客户端");
    }
}
