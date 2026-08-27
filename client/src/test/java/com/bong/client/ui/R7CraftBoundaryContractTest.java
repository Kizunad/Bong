package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7CraftBoundaryContractTest {
    private static final Path PRODUCTION_ROOT = R7SourceScan.productionRoot();

    @Test
    void craftScreenAndPanelDoNotReachBackIntoStoresOrNetwork() {
        for (String relative : List.of(
            "craft/CraftScreen.java",
            "craft/CraftRecipeListWidget.java"
        )) {
            String source = R7SourceScan.read(PRODUCTION_ROOT.resolve(relative));
            for (String forbidden : List.of(
                "CraftStore",
                "InventoryStateStore",
                "SkillSetStore",
                "ClientRequestSender",
                "ClientRequestProtocol",
                "ServerDataHandler"
            )) {
                assertFalse(source.contains(forbidden),
                    relative + " 只能消费 ViewModel/typed sink，不得重新依赖 " + forbidden);
            }
        }
    }

    @Test
    void controllerRemainsIndependentOfMinecraftAndUiBackends() {
        String controller = R7SourceScan.read(
            PRODUCTION_ROOT.resolve("craft/CraftScreenController.java")
        );
        for (String forbidden : List.of(
            "net.minecraft",
            "io.wispforest.owo",
            "CraftStore",
            "ClientRequestSender",
            "ClientRequestProtocol"
        )) {
            assertFalse(controller.contains(forbidden),
                "Craft controller 必须保持库无关，禁止依赖 " + forbidden);
        }
        assertTrue(controller.contains("UiStateSource<CraftScreenViewModel>"),
            "Craft controller 必须从 library-neutral source 读取状态");
        assertTrue(controller.contains("UiIntentSink<CraftIntent>"),
            "Craft controller 必须只暴露领域 typed intent sink");
    }

    @Test
    void concreteAdaptersOwnTheOnlyLegacyDependencies() {
        String stateAdapter = R7SourceScan.read(
            PRODUCTION_ROOT.resolve("craft/CraftUiStateSource.java")
        );
        String intentAdapter = R7SourceScan.read(
            PRODUCTION_ROOT.resolve("craft/CraftClientIntentSink.java")
        );

        assertTrue(stateAdapter.contains("CraftStore"));
        assertTrue(stateAdapter.contains("InventoryStateStore"));
        assertTrue(stateAdapter.contains("SkillSetStore"));
        assertFalse(stateAdapter.contains("ClientRequestSender"),
            "state adapter 不得拥有 C2S sender");
        assertTrue(intentAdapter.contains("ClientRequestSender"));
        assertFalse(intentAdapter.contains("CraftStore"),
            "intent adapter 不得反向读取 Store 判定 server 结果");
    }
}
