package com.bong.client;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;

/** P2 adjunct lifecycle ownership: disconnect must remain centralized in BongNetworkHandler. */
class AdjunctDisconnectOwnershipTest {
    private static final String[] BOOTSTRAPS_WITHOUT_DISCONNECT = {
        "environment/EnvironmentEffectController.java",
        "iris/IrisBootstrap.java",
        "combat/juice/CastFovController.java",
        "combat/CombatHudBootstrap.java",
        "movement/MovementKeybindings.java",
        "botany/BotanyHudBootstrap.java",
        "npc/NpcInteractionLogControls.java",
        "insight/InsightOfferScreenBootstrap.java",
        "ui/CultivationScreenBootstrap.java",
        "inventory/InspectScreenBootstrap.java",
        "inventory/LootContainerScreenBootstrap.java"
    };

    @Test
    void adjunctBootstrapsKeepProductionWiringButDoNotRegisterDistributedDisconnectCallbacks()
        throws Exception {
        String environment = source("environment/EnvironmentEffectController.java");
        String iris = source("iris/IrisBootstrap.java");
        String fov = source("combat/juice/CastFovController.java");

        assertTrue(environment.contains("ClientTickEvents.END_CLIENT_TICK.register(EnvironmentEffectController::tick)"),
            "EnvironmentEffectController 必须保留 tick production wiring");
        assertTrue(iris.contains("registerShaderStateChannel()"),
            "IrisBootstrap 必须保留 shader payload channel wiring");
        assertTrue(iris.contains("ClientTickEvents.END_CLIENT_TICK.register(client -> BongShaderState.tickInterpolate())"),
            "IrisBootstrap 必须保留 shader interpolation tick wiring");
        assertTrue(fov.contains("CastStateStore.addTransitionListener(CastFovController::onCastState)"),
            "CastFovController 必须保留 cast state production wiring");
        assertTrue(fov.contains("ClientEntityEvents.ENTITY_UNLOAD.register((entity, world) -> onEntityUnload(entity))"),
            "CastFovController 必须保留切世界 entity-unload teardown wiring");

        for (String path : BOOTSTRAPS_WITHOUT_DISCONNECT) {
            assertFalse(source(path).contains("ClientPlayConnectionEvents.DISCONNECT"),
                path + " 不得保留绕过 active-handler token gate 的分散 DISCONNECT 注册");
        }
    }

    @Test
    void centralizedDisconnectHelperOwnsEachAdjunctExactlyOnce() throws Exception {
        String networkHandler = source("BongNetworkHandler.java");
        int helperStart = networkHandler.indexOf("static void clearClientStateOnDisconnect()");
        assertTrue(helperStart >= 0, "必须存在 token-gated 中央断线 helper");
        int helperEnd = networkHandler.indexOf("\n    private static", helperStart + 1);
        assertTrue(helperEnd > helperStart, "必须能圈定中央断线 helper");
        String helper = networkHandler.substring(helperStart, helperEnd);

        for (String call : new String[] {
            "EnvironmentEffectController.clearOnDisconnect()",
            "BongShaderState.clearOnDisconnect()",
            "CastFovController.teardown()",
            "CombatJuiceSystem.clearOnDisconnect()",
            "CombatHudBootstrap.clearOnDisconnect()",
            "MovementKeybindings.clearOnDisconnect()",
            "BotanyHudBootstrap.clearOnDisconnect()"
        }) {
            assertTrue(helper.contains(call), "中央断线 helper 必须接入 adjunct：" + call);
            assertTrue(helper.indexOf(call) == helper.lastIndexOf(call),
                "中央断线 helper 必须恰好调用一次 adjunct：" + call);
        }
    }

    @Test
    void productionDisconnectHooksDoNotDelegateToTestResets() throws Exception {
        for (String path : new String[] {
            "environment/EnvironmentEffectController.java",
            "iris/BongShaderState.java",
            "combat/juice/CastFovController.java",
            "combat/CombatHudBootstrap.java",
            "combat/CombatKeybindings.java",
            "movement/MovementKeybindings.java",
            "botany/BotanyHudBootstrap.java",
            "botany/BotanyDragState.java"
        }) {
            String hook = productionHook(source(path), "clearOnDisconnect");
            assertFalse(hook.contains("resetForTest"),
                path + " 的生产断线清理不得调用 resetForTests/resetForTest");
            assertFalse(hook.contains("clearForTest"),
                path + " 的生产断线清理不得调用 clearForTests");
        }
    }

    private static String source(String relativePath) throws Exception {
        Path workingDirectory = Path.of("").toAbsolutePath().normalize();
        Path clientRoot = Files.isDirectory(workingDirectory.resolve("src"))
            ? workingDirectory
            : workingDirectory.resolve("client");
        Path source = clientRoot.resolve("src/main/java/com/bong/client").resolve(relativePath);
        assertTrue(Files.exists(source), "必须存在 lifecycle source：" + source.toAbsolutePath());
        return Files.readString(source);
    }

    private static String productionHook(String source, String methodName) {
        int start = source.indexOf("public static void " + methodName + "()");
        assertTrue(start >= 0, "必须暴露生产 lifecycle hook：" + methodName);
        int bodyStart = source.indexOf('{', start);
        assertTrue(bodyStart >= 0, "生产 lifecycle hook 必须有方法体：" + methodName);
        int depth = 0;
        for (int index = bodyStart; index < source.length(); index++) {
            char current = source.charAt(index);
            if (current == '{') {
                depth++;
            } else if (current == '}' && --depth == 0) {
                return source.substring(start, index + 1);
            }
        }
        throw new AssertionError("无法圈定生产 lifecycle hook：" + methodName);
    }
}
