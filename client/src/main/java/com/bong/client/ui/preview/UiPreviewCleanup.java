package com.bong.client.ui.preview;

import java.util.Objects;

/** 按顺序完成预览清理，并保留全部清理失败以便诊断。 */
final class UiPreviewCleanup {
    private UiPreviewCleanup() {
    }

    static void run(Runnable... cleanups) {
        Objects.requireNonNull(cleanups, "cleanups 不能为空");
        Throwable primary = null;
        for (Runnable cleanup : cleanups) {
            Objects.requireNonNull(cleanup, "cleanup 不能为空");
            try {
                cleanup.run();
            } catch (RuntimeException | Error failure) {
                if (primary == null) {
                    primary = failure;
                } else if (failure != primary) {
                    primary.addSuppressed(failure);
                }
            }
        }
        if (primary instanceof RuntimeException failure) {
            throw failure;
        }
        if (primary instanceof Error failure) {
            throw failure;
        }
    }
}
