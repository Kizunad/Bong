package com.bong.client;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;

/** P2 adjunct lifecycle ownership: disconnect must remain centralized in BongNetworkHandler. */
class AdjunctDisconnectOwnershipTest {
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

        assertFalse(environment.contains("ClientPlayConnectionEvents.DISCONNECT"),
            "EnvironmentEffectController 不得保留分散 DISCONNECT 注册；未来应由 token-gated 中央入口调用 clearOnDisconnect");
        assertFalse(iris.contains("ClientPlayConnectionEvents.DISCONNECT"),
            "IrisBootstrap 不得保留分散 DISCONNECT 注册；未来应由 token-gated 中央入口调用 BongShaderState.clearOnDisconnect");
        assertFalse(fov.contains("ClientPlayConnectionEvents.DISCONNECT"),
            "CastFovController 不得保留分散 DISCONNECT 注册；未来应由 token-gated 中央入口调用 clearOnDisconnect");
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
            "MovementKeybindings.clearOnDisconnect()"
        }) {
            assertTrue(helper.contains(call), "中央断线 helper 必须接入 adjunct：" + call);
            assertTrue(helper.indexOf(call) == helper.lastIndexOf(call),
                "中央断线 helper 必须恰好调用一次 adjunct：" + call);
        }
    }

    @Test
    void productionDisconnectHooksDoNotDelegateToTestResets() throws Exception {
        String environment = source("environment/EnvironmentEffectController.java");
        String shader = source("iris/BongShaderState.java");
        String fov = source("combat/juice/CastFovController.java");

        assertFalse(productionHook(environment, "clearOnDisconnect").contains("resetForTest"),
            "环境生产断线清理不得调用 resetForTests");
        assertFalse(productionHook(shader, "clearOnDisconnect").contains("resetForTest"),
            "shader 生产断线清理不得调用 resetForTests");
        assertFalse(productionHook(fov, "clearOnDisconnect").contains("resetForTest"),
            "FOV 生产断线清理不得调用 resetForTests");
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
        int nextMethod = source.indexOf("\n    public static", start + 1);
        return nextMethod < 0 ? source.substring(start) : source.substring(start, nextMethod);
    }
}
