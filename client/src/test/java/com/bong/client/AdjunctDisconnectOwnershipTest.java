package com.bong.client;

import com.bong.client.lifecycle.ClientStoreScopeManifest;
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
    void centralizedDisconnectHelperOwnsEachAdjunctExactlyOnce() throws Exception {
        String networkHandler = source("BongNetworkHandler.java");
        String centralHelper = methodBody(
            networkHandler,
            "static void clearClientStateOnDisconnect()"
        );
        String adjunctHelper = methodBody(
            networkHandler,
            "private static void runAdjunctDisconnectTeardown()"
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

        for (String registration : new String[] {
            "() -> EnvironmentEffectController.clearOnDisconnect()",
            "() -> BongShaderState.clearOnDisconnect()",
            "() -> CastFovController.clearOnDisconnect()",
            "() -> CombatJuiceSystem.clearOnDisconnect()",
            "() -> CombatHudBootstrap.clearOnDisconnect()",
            "() -> MovementKeybindings.clearOnDisconnect()",
            "() -> BotanyHudBootstrap.clearOnDisconnect()",
            "() -> TechniquesListPanel.clearOnDisconnect()",
            "() -> WeaponTreasurePanel.clearOnDisconnect()",
            "() -> HomeSequence.clearOnDisconnect()",
            "() -> InventoryMoveRejectedHandler.clearOnDisconnect()",
            "() -> PillBuffHudPlanner.clearOnDisconnect()",
            "() -> MorphCastVignetteState.clearOnDisconnect()",
            "() -> SeasonVisualController.clearOnDisconnect()",
            "() -> ScreenTransitionController.clearOnDisconnect()",
            "() -> WorldVfxDemoBootstrap.clearOnDisconnect()",
            "() -> DeadDropBreakPlayer.clearOnDisconnect()",
            "() -> NpcFootstepAudioController.clearOnDisconnect()",
            "() -> BongAnimationRegistry.clearOnDisconnect()"
        }) {
            assertTrue(
                adjunctHelper.contains(registration),
                "唯一 adjunct owner 必须接入稳定身份 adjunct：" + registration
            );
            assertEquals(
                networkHandler.indexOf(registration),
                networkHandler.lastIndexOf(registration),
                "整个 production owner 必须恰好登记一次 adjunct：" + registration
            );
        }
    }

    @Test
    void allowlistedAdjunctImplementationsDoNotReferenceRegistryManagedStores() throws Exception {
        String[][] hooks = {
            {"environment/EnvironmentEffectController.java", "EnvironmentEffectController", "clearOnDisconnect"},
            {"iris/BongShaderState.java", "BongShaderState", "clearOnDisconnect"},
            {"combat/juice/CameraShakeController.java", "CameraShakeController.Shake", "none"},
            {"combat/juice/EntityTintController.java", "EntityTintController.Tint", "none"},
            {"combat/juice/KillJuiceController.java", "KillJuiceController.KillState", "none"},
            {"combat/juice/KillJuiceController.java", "KillJuiceController.MultiKillState", "empty"},
            {"combat/juice/CombatJuiceSystem.java", "CombatJuiceSystem.LastCommand", "empty"},
            {"combat/juice/CombatJuiceSystem.java", "CombatJuiceSystem.Overlay", "none"},
            {"loop/HomeSequence.java", "HomeSequence.State", "away"},
            {"season/SeasonBreakthroughOverlayHud.java", "SeasonBreakthroughOverlayHud.ActivePulse", "empty"},
            {"combat/juice/CastFovController.java", "CastFovController", "clearOnDisconnect"},
            {"combat/juice/CombatJuiceSystem.java", "CombatJuiceSystem", "clearOnDisconnect"},
            {"combat/CombatHudBootstrap.java", "CombatHudBootstrap", "clearOnDisconnect"},
            {"movement/MovementKeybindings.java", "MovementKeybindings", "clearOnDisconnect"},
            {"botany/BotanyHudBootstrap.java", "BotanyHudBootstrap", "clearOnDisconnect"},
            {"combat/inspect/TechniquesListPanel.java", "TechniquesListPanel", "clearOnDisconnect"},
            {"combat/inspect/WeaponTreasurePanel.java", "WeaponTreasurePanel", "clearOnDisconnect"},
            {"loop/HomeSequence.java", "HomeSequence", "clearOnDisconnect"},
            {"network/InventoryMoveRejectedHandler.java", "InventoryMoveRejectedHandler", "clearOnDisconnect"},
            {"hud/PillBuffHudPlanner.java", "PillBuffHudPlanner", "clearOnDisconnect"},
            {"hud/MorphCastVignetteState.java", "MorphCastVignetteState", "clearOnDisconnect"},
            {"season/SeasonVisualController.java", "SeasonVisualController", "clearOnDisconnect"},
            {"ui/ScreenTransitionController.java", "ScreenTransitionController", "clearOnDisconnect"},
            {"visual/particle/WorldVfxDemoBootstrap.java", "WorldVfxDemoBootstrap", "clearOnDisconnect"},
            {"visual/particle/DeadDropBreakPlayer.java", "DeadDropBreakPlayer", "clearOnDisconnect"},
            {"audio/NpcFootstepAudioController.java", "NpcFootstepAudioController", "clearOnDisconnect"},
            {"animation/BongAnimationRegistry.java", "BongAnimationRegistry", "clearOnDisconnect"},
            {"npc/NpcDialogueBubbleRenderer.java", "NpcDialogueBubbleRenderer", "clear"},
            {"audio/MusicStateMachine.java", "MusicStateMachine", "clearOnDisconnect"},
            {"audio/MusicStateMachine.java", "MusicStateMachine", "clear"},
            {"audio/MusicStateMachine.java", "MusicStateMachine", "stopActive", "1"},
            {"audio/MusicStateMachine.java", "MusicStateMachine", "instance"},
            {"audio/MusicStateMachine.java", "MusicStateMachine", "clearSeasonModifierOnDisconnect"},
            {"audio/SoundRecipePlayer.java", "SoundRecipePlayer", "clearOnDisconnect"},
            {"audio/SoundRecipePlayer.java", "SoundRecipePlayer", "setMusicState", "1"},
            {"audio/SoundRecipePlayer.java", "SoundRecipePlayer", "stop", "1"},
            {"audio/SoundRecipePlayer.java", "SoundRecipePlayer.ActiveLoop", "deactivateOwnedFlag"},
            {"network/AudioEventPayload.java", "AudioEventPayload.PlaySoundRecipe", "instanceId"},
            {"audio/AudioBusMixer.java", "AudioBusMixer", "setMusicState", "1"},
            {"environment/EnvironmentAudioLoopState.java", "EnvironmentAudioLoopState", "deactivate", "1"},
            {"audio/AudioBusMixer.java", "AudioBusMixer", "clearOnDisconnect"},
            {"audio/SoundSink.java", "SoundSink", "clearOnDisconnect"},
            {"audio/SoundSink.java", "SoundSink", "stop", "2"},
            {"audio/MinecraftSoundSink.java", "MinecraftSoundSink", "clearOnDisconnect"},
            {"audio/FadeableSoundInstance.java", "FadeableSoundInstance", "beginFadeOut", "1"},
            {"animation/BongAnimationPlayer.java", "BongAnimationPlayer", "clearOnDisconnect"},
            {"animation/AnimationLayerManager.java", "AnimationLayerManager", "clearOnDisconnect"},
            {"animation/BongPunchCombo.java", "BongPunchCombo", "clearOnDisconnect"},
            {"dandao/MutationVisualState.java", "MutationVisualState", "reset"},
            {"spider/SpiderDisguiseHandler.java", "SpiderDisguiseHandler", "clearOnDisconnect"},
            {"fauna/RatQiTierHandler.java", "RatQiTierHandler", "clearOnDisconnect"},
            {"daozhan/DaoZhanDisguiseHandler.java", "DaoZhanDisguiseHandler", "clearOnDisconnect"},
            {"era/EraAmbianceState.java", "EraAmbianceState", "reset"},
            {"hud/BongToast.java", "BongToast", "clearOnDisconnect"},
            {"BongNetworkHandler.java", "BongNetworkHandler", "runDisconnectCleanups", "1"},
            {"combat/juice/CameraShakeController.java", "CameraShakeController", "clear"},
            {"combat/juice/HitStopController.java", "HitStopController", "clearOnDisconnect"},
            {"combat/juice/CameraShakeController.java", "CameraShakeController", "clearOnDisconnect"},
            {"combat/juice/EntityTintController.java", "EntityTintController", "clearOnDisconnect"},
            {"combat/juice/KillJuiceController.java", "KillJuiceController", "clearOnDisconnect"},
            {"combat/CombatKeybindings.java", "CombatKeybindings", "clearOnDisconnect"},
            {"social/SparringInviteScreenBootstrap.java", "SparringInviteScreenBootstrap", "clearOnDisconnect"},
            {"botany/BotanyDragState.java", "BotanyDragState", "clearOnDisconnect"},
            {"season/SeasonBreakthroughOverlayHud.java", "SeasonBreakthroughOverlayHud", "clearOnDisconnect"},
            {"atmosphere/ZoneAtmosphereRenderer.java", "ZoneAtmosphereRenderer", "clearSeasonOverrideOnDisconnect"},
            {"environment/EnvironmentAudioLoopState.java", "EnvironmentAudioLoopState", "clearOnDisconnect"},
            {"environment/EnvironmentAudioController.java", "EnvironmentAudioController", "clearOnDisconnect"},
            {"environment/EnvironmentEffectRegistry.java", "EnvironmentEffectRegistry", "clear"},
            {"atmosphere/ZoneAtmosphereRenderer.java", "ZoneAtmosphereRenderer", "clear"},
            {"atmosphere/AshFootprintTracker.java", "AshFootprintTracker", "clear"},
            {"environment/EnvironmentFogController.java", "EnvironmentFogController", "clear"}
        };
        java.util.ArrayList<JavaLifecycleSourceInspector.AuditedLifecycleEntry> closure =
            new java.util.ArrayList<>();
        for (String[] hook : hooks) {
            closure.add(new JavaLifecycleSourceInspector.AuditedLifecycleEntry(
                hook[0],
                source(hook[0]),
                hook[1],
                hook[2],
                hook.length == 4 ? Integer.parseInt(hook[3]) : 0
            ));
        }
        JavaLifecycleSourceInspector.assertLifecycleClosureContainsNoStoreCleanupReferences(
            closure,
            ClientStoreScopeManifest.registryManagedSessionStores()
        );
    }

    @Test
    void adjunctStoreReferenceAuditRejectsHiddenStoreReferenceShapes() {
        for (String[] fixture : new String[][] {
            {
                """
                    package com.bong.client.hud;
                    final class AllowlistedAdjunct {
                        static void clearOnDisconnect() {
                            LootContainerStateStore.clearOnDisconnect();
                        }
                    }
                    """,
                "clearOnDisconnect"
            },
            {
                """
                    import static com.bong.client.hud.LootContainerStateStore.clearOnDisconnect;
                    final class AllowlistedAdjunct {
                        static void tearDown() {
                            clearOnDisconnect();
                        }
                    }
                    """,
                "tearDown"
            }
        }) {
            org.junit.jupiter.api.Assertions.assertThrows(
                AssertionError.class,
                () -> JavaLifecycleSourceInspector.assertMethodContainsNoStoreReferences(
                    fixture[0],
                    "AllowlistedAdjunct",
                    fixture[1],
                    java.util.Set.of("com.bong.client.hud.LootContainerStateStore")
                )
            );
        }
    }

    @Test
    void adjunctClosureRejectsLegacyCleanupHelpersCrossClassCallsAndWildcardImports() {
        for (String fixture : new String[] {
            """
                package com.example;
                import com.bong.client.hud.LootContainerStateStore;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() { tearDown(); }
                    static void tearDown() { LootContainerStateStore.clear(); }
                }
                """,
            """
                package com.example;
                import com.bong.client.hud.LootContainerStateStore;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() { tearDown(); }
                    static void tearDown() { LootContainerStateStore.clearAll(); }
                }
                """,
            """
                package com.example;
                import com.bong.client.hud.LootContainerStateStore;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() { tearDown(); }
                    static void tearDown() { LootContainerStateStore.reset(); }
                }
                """,
            """
                package com.example;
                import com.bong.client.hud.LootContainerStateStore;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() {
                        LootContainerStateStore store = null;
                        store.discardAllVisuals();
                    }
                }
                """,
            """
                package com.example;
                import com.bong.client.hud.LootContainerStateStore;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() { new LootContainerStateStore(); }
                }
                """,
            """
                package com.example;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() { Helper.tearDown(); }
                }
                final class Helper {
                    static void tearDown() { }
                }
                """,
            """
                package com.example;
                final class AllowlistedAdjunct {
                    private static final Helper HELPER = null;
                    static void clearOnDisconnect() { HELPER.tearDown(); }
                }
                final class Helper {
                    static void tearDown() { }
                }
                """,
            """
                package com.example;
                final class AllowlistedAdjunct {
                    private static final Helper helper = null;
                    static void clearOnDisconnect() { helper.tearDown(); }
                }
                final class Helper {
                    static void tearDown() { }
                }
                """,
            """
                package com.example;
                import static com.bong.client.hud.LootContainerStateStore.*;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() { clear(); }
                }
                """,
            """
                package com.example;
                import com.bong.client.hud.*;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() { }
                }
                """,
            """
                package com.example;
                import com.bong.client.hud.LootContainerStateStore;
                class BaseAdjunct {
                    static void clearInherited() {
                        LootContainerStateStore.clearOnDisconnect();
                    }
                }
                final class AllowlistedAdjunct extends BaseAdjunct {
                    static void clearOnDisconnect() { clearInherited(); }
                }
                """
        }) {
            org.junit.jupiter.api.Assertions.assertThrows(
                AssertionError.class,
                () -> JavaLifecycleSourceInspector.assertMethodClosureContainsNoStoreCleanupReferences(
                    fixture,
                    "AllowlistedAdjunct",
                    "clearOnDisconnect",
                    java.util.Set.of("com.bong.client.hud.LootContainerStateStore"),
                    java.util.Set.of()
                )
            );
        }
    }

    @Test
    void adjunctClosureRejectsNestedAndLocalVariableReceiversWithoutExplicitAudit() {
        for (String fixture : new String[] {
            """
                package com.example;
                final class AllowlistedAdjunct {
                    private static final Owner OWNER = null;
                    static void clearOnDisconnect() { OWNER.helper.tearDown(); }
                }
                final class Owner { Helper helper; }
                final class Helper { void tearDown() { } }
                """,
            """
                package com.example;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() {
                        Helper helper = null;
                        helper.tearDown();
                    }
                }
                final class Helper { void tearDown() { } }
                """
        }) {
            org.junit.jupiter.api.Assertions.assertThrows(
                AssertionError.class,
                () -> JavaLifecycleSourceInspector.assertMethodClosureContainsNoStoreCleanupReferences(
                    fixture,
                    "AllowlistedAdjunct",
                    "clearOnDisconnect",
                    java.util.Set.of("com.bong.client.hud.LootContainerStateStore"),
                    java.util.Set.of()
                )
            );
        }
    }

    @Test
    void adjunctClosureAllowsRecursiveLocalDataOnlyHelpers() {
        org.junit.jupiter.api.Assertions.assertDoesNotThrow(
            () -> JavaLifecycleSourceInspector.assertMethodClosureContainsNoStoreCleanupReferences(
                """
                    package com.example;
                    final class AllowlistedAdjunct {
                        static void clearOnDisconnect() { first(); }
                        static void first() { second(); }
                        static void second() { first(); }
                    }
                    """,
                "AllowlistedAdjunct",
                "clearOnDisconnect",
                java.util.Set.of("com.bong.client.hud.LootContainerStateStore"),
                java.util.Set.of()
            )
        );
    }

    @Test
    void adjunctClosureInfersCollectionLambdaReceiverTypes() {
        org.junit.jupiter.api.Assertions.assertDoesNotThrow(
            () -> JavaLifecycleSourceInspector.assertMethodClosureContainsNoStoreCleanupReferences(
                """
                    package com.example;
                    import java.util.List;
                    final class AllowlistedAdjunct {
                        private static final List<Payload> pending = null;
                        static void clearOnDisconnect() {
                            pending.removeIf(queued -> queued.instanceId() == 1L);
                        }
                    }
                    record Payload(long instanceId) { }
                    """,
                "AllowlistedAdjunct",
                "clearOnDisconnect",
                java.util.Set.of("com.bong.client.hud.LootContainerStateStore"),
                java.util.Set.of()
            )
        );
    }

    @Test
    void adjunctClosureRequiresExactCrossClassOwnerMethodAndArity() {
        String owner = "com.example.AllowlistedHelper.clearOnDisconnect/0";
        String entry = """
            package com.example;
            final class AllowlistedAdjunct {
                static void clearOnDisconnect() { AllowlistedHelper.clearOnDisconnect(); }
            }
            """;
        org.junit.jupiter.api.Assertions.assertDoesNotThrow(
            () -> JavaLifecycleSourceInspector.assertMethodClosureContainsNoStoreCleanupReferences(
                entry,
                "AllowlistedAdjunct",
                "clearOnDisconnect",
                java.util.Set.of("com.bong.client.hud.LootContainerStateStore"),
                java.util.Set.of(owner)
            )
        );

        for (String fixture : new String[] {
            """
                package com.example;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() { AllowlistedHelper.hiddenCleanup(); }
                }
                """,
            """
                package com.example;
                final class AllowlistedAdjunct {
                    static void clearOnDisconnect() { AllowlistedHelper.clearOnDisconnect(1); }
                }
                """
        }) {
            org.junit.jupiter.api.Assertions.assertThrows(
                AssertionError.class,
                () -> JavaLifecycleSourceInspector.assertMethodClosureContainsNoStoreCleanupReferences(
                    fixture,
                    "AllowlistedAdjunct",
                    "clearOnDisconnect",
                    java.util.Set.of("com.bong.client.hud.LootContainerStateStore"),
                    java.util.Set.of(owner)
                )
            );
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
