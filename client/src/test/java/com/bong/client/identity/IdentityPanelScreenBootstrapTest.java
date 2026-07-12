package com.bong.client.identity;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;

/**
 * plan-bughunt-client-identity-panel-stale-session-v1 — {@link IdentityPanelScreenBootstrap#onStoreChanged}
 * 的 null-safety 契约测试。
 *
 * <p>无头 JUnit 环境下没有真实渲染窗口，{@code MinecraftClient.getInstance()} 恒为 {@code null}
 * （全仓惯例：owo/vanilla screen 的 {@code MinecraftClient} 依赖部分不做完整 UI 树单测，见
 * {@code AlchemyScreenInventoryWiringTest} 文档注释）。这里只锁住"store 通知触发时，缺少
 * 真实 client 不能抛异常"这条防线——{@code IdentityPanelStateStore.clearOnDisconnect()} 在
 * server/agent 侧集成测试或工具脚本里也可能在没有真实 MinecraftClient 的语境下被触发。
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
