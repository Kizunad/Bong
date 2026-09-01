package com.bong.client.identity;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;

/**
 * 身份面板 store 通知的 null-safety 契约测试。
 *
 * <p>无头 JUnit 环境下没有真实渲染窗口，{@code MinecraftClient.getInstance()} 恒为 {@code null}；
 * 这里锁住断线或工具态通知不会因为缺少 client 而抛异常。
 */
final class IdentityPanelScreenBootstrapTest {
    @Test
    void onStoreChangedIsNoOpWithoutLiveMinecraftClient() {
        assertDoesNotThrow(
            () -> IdentityPanelScreenBootstrap.onStoreChanged(IdentityPanelState.empty()),
            "onStoreChanged 必须在 MinecraftClient.getInstance()==null 时 short-circuit，"
                + "不能因为 store 通知就尝试访问 client.currentScreen/client.setScreen 抛 NPE");
    }

    @Test
    void onStoreChangedIsNoOpWithoutLiveMinecraftClientForNonEmptyState() {
        assertDoesNotThrow(
            () -> IdentityPanelScreenBootstrap.onStoreChanged(new IdentityPanelState(
                1, 0L, 0L, java.util.List.of(new IdentityPanelEntry(1, "测试身份", 0, false, java.util.List.of())))),
            "非空 state 通知同样必须 null-safe，不依赖 state 内容判空");
    }
}
