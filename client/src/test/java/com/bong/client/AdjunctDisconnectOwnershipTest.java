package com.bong.client;

import com.bong.client.lifecycle.JavaLifecycleSourceInspector;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
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

        List<String> disconnectOwners;
        try (var sources = Files.walk(productionSourceRoot())) {
            disconnectOwners = sources
                .filter(Files::isRegularFile)
                .filter(path -> path.getFileName().toString().endsWith(".java"))
                .filter(path -> read(path).contains("ClientPlayConnectionEvents.DISCONNECT"))
                .map(path -> productionSourceRoot().relativize(path).toString().replace('\\', '/'))
                .sorted()
                .toList();
        }
        assertEquals(
            List.of("BongNetworkHandler.java"),
            disconnectOwners,
            "全部 production Java 源码中只能由 BongNetworkHandler 注册 DISCONNECT；任何分散入口都会绕过 active-handler token gate"
        );
    }

    @Test
    void centralizedDisconnectHelperDelegatesToAdjunctOwnerExactlyOnce() throws Exception {
        String networkHandler = source("BongNetworkHandler.java");
        String centralHelper = methodBody(
            networkHandler,
            "static void clearClientStateOnDisconnect()"
        );
        String adjunctDelegation = "runAdjunctDisconnectTeardown()";
        assertTrue(
            centralHelper.contains(adjunctDelegation),
            "token-gated 中央断线 helper 必须直接委托唯一 adjunct owner"
        );
        assertEquals(
            centralHelper.indexOf(adjunctDelegation),
            centralHelper.lastIndexOf(adjunctDelegation),
            "token-gated 中央断线 helper 必须恰好委托一次 adjunct owner"
        );
    }

    @Test
    void centralizedDisconnectHelperPinsEveryReviewedAdjunctRegistrationInOrder() throws Exception {
        assertEquals(
            List.of(
                "()->EnvironmentEffectController.clearOnDisconnect()",
                "()->BongShaderState.clearOnDisconnect()",
                "()->CastFovController.clearOnDisconnect()",
                "()->CombatJuiceSystem.clearOnDisconnect()",
                "()->CombatHudBootstrap.clearOnDisconnect()",
                "()->MovementKeybindings.clearOnDisconnect()",
                "()->BotanyHudBootstrap.clearOnDisconnect()",
                "()->TechniquesListPanel.clearOnDisconnect()",
                "()->WeaponTreasurePanel.clearOnDisconnect()",
                "()->HomeSequence.clearOnDisconnect()",
                "()->InventoryMoveRejectedHandler.clearOnDisconnect()",
                "()->PillBuffHudPlanner.clearOnDisconnect()",
                "()->MorphCastVignetteState.clearOnDisconnect()",
                "()->SeasonVisualController.clearOnDisconnect()",
                "()->ScreenTransitionController.clearOnDisconnect()",
                "()->WorldVfxDemoBootstrap.clearOnDisconnect()",
                "()->DeadDropBreakPlayer.clearOnDisconnect()",
                "()->NpcFootstepAudioController.clearOnDisconnect()",
                "()->BongAnimationRegistry.clearOnDisconnect()",
                "()->NpcDialogueBubbleRenderer.clear()",
                "()->com.bong.client.audio.MusicStateMachine.clearOnDisconnect()",
                "()->SoundRecipePlayer.instance().clearOnDisconnect()",
                "()->BongAnimationPlayer.clearOnDisconnect()",
                "()->AnimationLayerManager.clearOnDisconnect()",
                "()->LowerBodyGaitController.clearOnDisconnect()",
                "()->BongPunchCombo.clearOnDisconnect()",
                "()->MutationVisualState.reset()",
                "()->SpiderDisguiseHandler.clearOnDisconnect()",
                "()->RatQiTierHandler.clearOnDisconnect()",
                "()->DaoZhanDisguiseHandler.clearOnDisconnect()",
                "()->com.bong.client.era.EraAmbianceState.reset()",
                "()->BongToast.clearOnDisconnect()"
            ),
            JavaLifecycleSourceInspector.disconnectCleanupRegistrations(source("BongNetworkHandler.java")),
            "中央 adjunct 注册表必须与逐站点审阅清单完全一致；新增、删除、重排或改写都需显式更新合同"
        );
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
            "botany/BotanyDragState.java",
            "animation/BongAnimationRegistry.java",
            "audio/NpcFootstepAudioController.java",
            "combat/inspect/TechniquesListPanel.java",
            "combat/inspect/WeaponTreasurePanel.java",
            "hud/MorphCastVignetteState.java",
            "hud/PillBuffHudPlanner.java",
            "loop/HomeSequence.java",
            "network/InventoryMoveRejectedHandler.java",
            "season/SeasonVisualController.java",
            "ui/ScreenTransitionController.java",
            "visual/particle/DeadDropBreakPlayer.java",
            "visual/particle/WorldVfxDemoBootstrap.java"
        }) {
            String hook = productionHook(source(path), "clearOnDisconnect");
            assertFalse(hook.contains("resetForTest"),
                path + " 的生产断线清理不得调用 resetForTests/resetForTest");
            assertFalse(hook.contains("clearForTest"),
                path + " 的生产断线清理不得调用 clearForTests");
        }
    }

    private static String source(String relativePath) throws Exception {
        Path source = productionSourceRoot().resolve(relativePath);
        assertTrue(Files.exists(source), "必须存在 lifecycle source：" + source.toAbsolutePath());
        return read(source);
    }

    private static Path productionSourceRoot() {
        Path workingDirectory = Path.of("").toAbsolutePath().normalize();
        Path clientRoot = Files.isDirectory(workingDirectory.resolve("src"))
            ? workingDirectory
            : workingDirectory.resolve("client");
        return clientRoot.resolve("src/main/java/com/bong/client");
    }

    private static String read(Path source) {
        try {
            return Files.readString(source);
        } catch (java.io.IOException exception) {
            throw new AssertionError("无法读取 production source：" + source.toAbsolutePath(), exception);
        }
    }

    private static String methodBody(String source, String declaration) {
        int start = source.indexOf(declaration);
        assertTrue(start >= 0, "必须存在 production lifecycle helper：" + declaration);
        int bodyStart = source.indexOf('{', start);
        assertTrue(bodyStart >= 0, "production lifecycle helper 必须有方法体：" + declaration);
        int depth = 0;
        for (int index = bodyStart; index < source.length(); index++) {
            char current = source.charAt(index);
            if (current == '{') {
                depth++;
            } else if (current == '}' && --depth == 0) {
                return source.substring(start, index + 1);
            }
        }
        throw new AssertionError("无法圈定 production lifecycle helper：" + declaration);
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
