package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.nio.file.Path;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class R7P3StateIntentBoundaryContractTest {
    private static final Path PRODUCTION_ROOT = R7SourceScan.productionRoot();

    @Test
    void targetScreensDoNotReachIntoStoresOrNetworkTransport() {
        Map<String, String[]> forbidden = Map.of(
            "alchemy/AlchemyScreen.java", new String[]{
                "AlchemyAttemptHistoryStore", "AlchemyFurnaceStore", "AlchemyOutcomeForecastStore",
                "AlchemySessionStore", "ContaminationWarningStore", "InventoryMetaStore", "RecipeScrollStore",
                "InventoryStateStore", "SkillSetStore", "ClientRequestSender", "ClientRequestProtocol"
            },
            "social/TradeOfferScreen.java", new String[]{
                "InventoryStateStore", "ClientRequestSender", "ClientRequestProtocol", ".tradeOffer()", ".clearTradeOffer("
            },
            "inventory/LootContainerScreen.java", new String[]{
                "InventoryStateStore", "ClientRequestSender", "ClientRequestProtocol", ".current()"
            },
            "inventory/LootContainerPanel.java", new String[]{
                "InventoryStateStore", "ClientRequestSender", "ClientRequestProtocol", ".current()"
            }
        );
        forbidden.forEach((relative, tokens) -> {
            String source = R7SourceScan.read(PRODUCTION_ROOT.resolve(relative));
            for (String token : tokens) {
                assertFalse(source.contains(token), relative + " 不得直接依赖 " + token);
            }
        });
    }

    @Test
    void controllersStayLibraryNeutralAndExposeTypedSinks() {
        for (String relative : new String[]{
            "alchemy/AlchemyScreenController.java",
            "social/TradeOfferScreenController.java",
            "inventory/LootContainerScreenController.java"
        }) {
            String source = R7SourceScan.read(PRODUCTION_ROOT.resolve(relative));
            assertFalse(source.contains("net.minecraft"), relative + " controller 不得依赖 Minecraft");
            assertFalse(source.contains("io.wispforest.owo"), relative + " controller 不得依赖 owo");
            assertFalse(source.contains("ClientRequestSender"), relative + " controller 不得依赖 sender");
            assertTrue(source.contains("UiIntentSink"), relative + " 必须暴露 typed sink");
            assertTrue(source.contains("UiStateSource"), relative + " 必须消费 UiStateSource");
        }
    }

    @Test
    void screensCloseTheirScopeWhenMinecraftRemovesThem() {
        for (String relative : new String[]{
            "alchemy/AlchemyScreen.java",
            "social/TradeOfferScreen.java",
            "inventory/LootContainerScreen.java"
        }) {
            String source = R7SourceScan.read(PRODUCTION_ROOT.resolve(relative));
            assertTrue(source.contains("void removed()"), relative + " 必须覆盖 Minecraft removed 生命周期钩子");
            assertTrue(source.contains("controller.onClose()"), relative + " removed/close 必须关闭 controller");
        }
    }

    @Test
    void lootMountReplacesAnOlderSessionInsteadOfKeepingStalePanel() {
        String inspect = R7SourceScan.read(PRODUCTION_ROOT.resolve("inventory/InspectScreen.java"));
        assertTrue(inspect.contains("lootPanel.sessionId() == open.sessionId()"),
            "Loot 宿主必须按 session identity 判断是否可复用 panel");
        assertTrue(inspect.contains("unmountLootPanel();"),
            "Loot session 切换时必须先卸载旧 panel");
    }
}
