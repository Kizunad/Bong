package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.io.IOException;

import static org.junit.jupiter.api.Assertions.assertTrue;

/** 锁住暗器注入屏从生产入口加载，而不是只存在于预览/测试路径。 */
final class ForgeCarrierProductionEntryTest {
    @Test
    void existingForgeEntryUsesCarrierBootstrapFactory() throws IOException {
        String bootstrap = R7SourceScan.read(
            R7SourceScan.productionRoot().resolve("forge/ForgeScreenBootstrap.java"));
        String screen = R7SourceScan.read(
            R7SourceScan.productionRoot().resolve("forge/ForgeScreen.java"));

        assertTrue(bootstrap.contains("client.setScreen(ForgeCarrierScreenBootstrap.create());"),
            "生产暗器入口必须通过 ForgeCarrierScreenBootstrap.create() 组装网络 sink");
        assertTrue(screen.contains("ForgeScreenBootstrap.requestOpenForgeCarrierScreen(client);"),
            "原有 ForgeScreen 打开入口必须显式路由到暗器注入 bootstrap");
        assertTrue(screen.contains("GLFW.GLFW_KEY_C"),
            "暗器注入必须保留一个可达的锻炉界面入口");
    }
}
