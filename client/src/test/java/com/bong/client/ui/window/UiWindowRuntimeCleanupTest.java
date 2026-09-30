package com.bong.client.ui.window;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

class UiWindowRuntimeCleanupTest {
    @Test
    void alchemyCleanupRunsAndIsSuppressedWhenForgeCleanupFails() {
        RuntimeException forgeFailure = new RuntimeException("forge close failed");
        IllegalStateException alchemyFailure = new IllegalStateException("alchemy close failed");
        List<String> calls = new ArrayList<>();

        Throwable actual = assertThrows(
            Throwable.class,
            () -> UiWindowRuntime.closeOwnedResources(
                () -> {
                    calls.add("forge");
                    throw forgeFailure;
                },
                () -> {
                    calls.add("alchemy");
                    throw alchemyFailure;
                }
            )
        );

        assertSame(forgeFailure, actual, "forge 的首个关闭异常必须保留为主异常");
        assertEquals(List.of("forge", "alchemy"), calls,
            "forge 关闭失败不能跳过 alchemy 清理");
        assertEquals(List.of(alchemyFailure), List.of(actual.getSuppressed()),
            "alchemy 关闭异常必须作为 suppressed 附加到首个异常");
    }
}
