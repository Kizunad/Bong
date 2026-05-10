package com.bong.client.ui;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

public final class ScreenTransitionController {
    private static volatile ActiveTransition activeTransition;
    private static volatile boolean applyingDirectly;
    private static volatile int cancelledTransitions;
    private static volatile boolean registered;

    private ScreenTransitionController() {
    }

    public static void register() {
        if (registered) {
            return;
        }
        registered = true;
        ScreenTransitionRegistry.bootstrapDefaults();
        ClientTickEvents.END_CLIENT_TICK.register(ScreenTransitionController::tick);
    }

    public static boolean interceptSetScreen(MinecraftClient client, Screen nextScreen) {
        if (client == null || applyingDirectly) {
            return false;
        }
        if (!UiTransitionSettings.enabled()) {
            clearActiveTransition();
            return false;
        }
        Screen oldScreen = client.currentScreen;
        if (oldScreen == nextScreen) {
            return false;
        }

        TransitionConfig.TransitionSpec spec = ScreenTransitionRegistry.resolve(oldScreen, nextScreen);
        int durationMs = UiTransitionSettings.durationFor(spec.durationMs());
        if (!spec.animates() || durationMs == 0) {
            clearActiveTransition();
            return false;
        }

        ActiveTransition previous = activeTransition;
        if (previous != null) {
            previous.handle().cancel();
            cancelledTransitions++;
        }

        long now = ScreenTransition.nowMillis();
        ScreenTransition.TransitionHandle handle = ScreenTransition.play(
            oldScreen,
            nextScreen,
            spec.type(),
            durationMs,
            spec.easing(),
            () -> applyDirect(client, nextScreen)
        );
        activeTransition = new ActiveTransition(handle, spec, now);
        return true;
    }

    public static void tick(MinecraftClient client) {
        ActiveTransition active = activeTransition;
        if (active == null || client == null) {
            return;
        }
        int width = client.getWindow() == null ? 0 : client.getWindow().getScaledWidth();
        int height = client.getWindow() == null ? 0 : client.getWindow().getScaledHeight();
        if (active.handle().sample(ScreenTransition.nowMillis(), width, height).finished()) {
            activeTransition = null;
            active.handle().complete();
        }
    }

    public static void cancelAndClose(MinecraftClient client) {
        clearActiveTransition();
        if (client != null) {
            applyDirect(client, null);
        }
    }

    public static boolean inputLocked() {
        ActiveTransition active = activeTransition;
        if (active == null) {
            return false;
        }
        return active.handle().sample(ScreenTransition.nowMillis(), 1, 1).inputLocked();
    }

    public static ActiveTransition activeTransition() {
        return activeTransition;
    }

    static int cancelledTransitionsForTests() {
        return cancelledTransitions;
    }

    static void resetForTests() {
        activeTransition = null;
        applyingDirectly = false;
        cancelledTransitions = 0;
    }

    private static void applyDirect(MinecraftClient client, Screen screen) {
        applyingDirectly = true;
        try {
            client.setScreen(screen);
        } finally {
            applyingDirectly = false;
        }
    }

    private static void clearActiveTransition() {
        ActiveTransition active = activeTransition;
        if (active != null) {
            active.handle().cancel();
            cancelledTransitions++;
            activeTransition = null;
        }
    }

    public record ActiveTransition(
        ScreenTransition.TransitionHandle handle,
        TransitionConfig.TransitionSpec spec,
        long startedAtMs
    ) {
    }
}
