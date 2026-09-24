package com.bong.client.ui.preview;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

class UiPreviewCleanupTest {
    @Test
    void runsEveryCleanupInOrderAndPreservesAllFailures() {
        List<String> calls = new ArrayList<>();
        IllegalStateException primary = new IllegalStateException("manager reset failed");
        IllegalArgumentException secondary = new IllegalArgumentException("inventory reset failed");
        AssertionError finalFailure = new AssertionError("preview end failed");

        Throwable actual = assertThrows(
            Throwable.class,
            () -> UiPreviewCleanup.run(
                () -> {
                    calls.add("manager");
                    throw primary;
                },
                () -> {
                    calls.add("inventory");
                    throw secondary;
                },
                () -> {
                    calls.add("preview");
                    throw finalFailure;
                }
            )
        );

        assertSame(primary, actual, "首个清理异常必须作为主异常抛出");
        assertEquals(
            List.of("manager", "inventory", "preview"),
            calls,
            "首个清理失败不得跳过后续清理步骤"
        );
        assertEquals(
            List.of(secondary, finalFailure),
            List.of(actual.getSuppressed()),
            "后续清理异常必须按发生顺序保留为 suppressed"
        );
    }
}
