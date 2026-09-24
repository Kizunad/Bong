package com.bong.client.ui;

import com.bong.client.craft.CraftScreenBootstrap;
import com.bong.client.ui.bootstrap.UiBootstrapModule;
import com.bong.client.ui.bootstrap.UiBootstrapRegistry;
import com.bong.client.ui.bootstrap.UiRuntime;
import com.bong.client.ui.window.UiWindowRuntime;

import java.util.Objects;
import java.util.Set;

/** P2 reference slice 的生产 bootstrap 图；其余 UI 模块按后续阶段逐批迁入。 */
public final class ClientUiBootstrap {
    private static final String SCREEN_TRANSITION = "screen_transition";
    private static final String CRAFT_SCREEN = "craft_screen";
    private static final String WINDOW_MANAGER = "window_manager";
    private static final UiRuntime RUNTIME = new UiRuntime() {
    };
    private static final UiBootstrapRegistry REGISTRY = referenceRegistry(
        ScreenTransitionController::register,
        CraftScreenBootstrap::register,
        UiWindowRuntime::initialize
    );

    private ClientUiBootstrap() {
    }

    public static void registerScreenTransition() {
        REGISTRY.register(SCREEN_TRANSITION, RUNTIME);
    }

    public static void registerCraftScreen() {
        REGISTRY.register(CRAFT_SCREEN, RUNTIME);
    }

    static UiBootstrapRegistry referenceRegistry(Runnable transition, Runnable craft, Runnable windows) {
        Objects.requireNonNull(transition, "transition must not be null");
        Objects.requireNonNull(craft, "craft must not be null");
        Objects.requireNonNull(windows, "windows must not be null");
        UiBootstrapRegistry registry = new UiBootstrapRegistry();
        registry.add(module(SCREEN_TRANSITION, Set.of(WINDOW_MANAGER), transition));
        registry.add(module(CRAFT_SCREEN, Set.of(SCREEN_TRANSITION), craft));
        registry.add(module(WINDOW_MANAGER, Set.of(), windows));
        return registry;
    }

    private static UiBootstrapModule module(String id, Set<String> dependencies, Runnable action) {
        return new UiBootstrapModule() {
            @Override
            public String id() {
                return id;
            }

            @Override
            public Set<String> dependencies() {
                return dependencies;
            }

            @Override
            public void register(UiRuntime runtime) {
                action.run();
            }
        };
    }
}
