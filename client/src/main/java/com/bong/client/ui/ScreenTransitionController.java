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
            cancelActiveTransitionForReplacement(nextScreen);
            return false;
        }
        Screen oldScreen = client.currentScreen;
        if (clearActiveTransitionIfSameScreen(oldScreen, nextScreen)) {
            return false;
        }

        TransitionConfig.TransitionSpec spec = ScreenTransitionRegistry.resolve(oldScreen, nextScreen);
        int durationMs = UiTransitionSettings.durationFor(spec.durationMs());
        if (!spec.animates() || durationMs == 0) {
            cancelActiveTransitionForReplacement(nextScreen);
            return false;
        }

        cancelActiveTransitionForReplacement(nextScreen);

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
        Screen currentScreen = currentScreenForCancellation(client);
        Screen pendingScreen = pendingScreenForCancellation();
        clearActiveTransition();
        closeCurrentThenSettlePending(
            currentScreen,
            pendingScreen,
            () -> {
                if (client != null) {
                    applyDirect(client, null);
                }
            }
        );
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

    public static Screen pendingScreen() {
        return pendingScreenForCancellation();
    }

    /** 精确取消指定 pending screen；不关闭当前 screen，也不影响后来替换的 transition。 */
    public static boolean cancelPendingOpen(Screen expected) {
        ActiveTransition active = activeTransition;
        if (expected == null
            || active == null
            || active.handle().completed()
            || active.handle().newScreen() != expected) {
            return false;
        }
        active.handle().cancel();
        cancelledTransitions++;
        activeTransition = null;
        if (expected instanceof PendingOpenCancellationHandler handler) {
            handler.onPendingOpenCancelled();
        }
        return true;
    }

    static int cancelledTransitionsForTests() {
        return cancelledTransitions;
    }

    static void setActiveTransitionForTests(ActiveTransition transition) {
        activeTransition = transition;
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

    static boolean clearActiveTransitionIfSameScreen(Screen oldScreen, Screen nextScreen) {
        if (oldScreen != nextScreen) {
            return false;
        }
        cancelActiveTransitionForReplacement(nextScreen);
        return true;
    }

    static void cancelActiveTransitionForReplacement(Screen replacementScreen) {
        ActiveTransition cancelled = clearActiveTransition();
        if (cancelled == null) {
            return;
        }
        Screen pendingScreen = cancelled.handle().completed()
            ? null
            : cancelled.handle().newScreen();
        if (pendingScreen instanceof PendingOpenCancellationHandler handler
            && !handler.continuesWith(replacementScreen)) {
            handler.onPendingOpenCancelled();
        }
    }

    private static ActiveTransition clearActiveTransition() {
        ActiveTransition active = activeTransition;
        if (active != null) {
            active.handle().cancel();
            cancelledTransitions++;
            activeTransition = null;
        }
        return active;
    }

    private static Screen pendingScreenForCancellation() {
        ActiveTransition active = activeTransition;
        if (active == null || active.handle().completed()) {
            return null;
        }
        return active.handle().newScreen();
    }

    private static Screen currentScreenForCancellation(MinecraftClient client) {
        if (client != null) {
            return client.currentScreen;
        }
        ActiveTransition active = activeTransition;
        if (active == null || active.handle().completed()) {
            return null;
        }
        return active.handle().oldScreen();
    }

    static void closeCurrentThenSettlePending(
        Screen currentScreen,
        Screen pendingScreen,
        Runnable closeCurrentScreen
    ) {
        if (closeCurrentScreen != null) {
            closeCurrentScreen.run();
        }
        if (currentScreen instanceof CurrentScreenCancellationHandler handler) {
            handler.onCurrentScreenCancelled();
        }
        if (pendingScreen instanceof PendingOpenCancellationHandler handler) {
            handler.onPendingOpenCancelled();
        }
    }

    /** 尚未完成开屏的 screen 若携带协议终态，可在 Esc 或后续 screen 覆盖时幂等收口。 */
    public interface PendingOpenCancellationHandler {
        void onPendingOpenCancelled();

        default boolean continuesWith(Screen replacementScreen) {
            return replacementScreen == this;
        }
    }

    /**
     * 转场取消会直接移除的当前 screen 若携带协议终态，可实现此接口在移除后幂等收口。
     */
    public interface CurrentScreenCancellationHandler {
        void onCurrentScreenCancelled();
    }

    public record ActiveTransition(
        ScreenTransition.TransitionHandle handle,
        TransitionConfig.TransitionSpec spec,
        long startedAtMs
    ) {
    }
}
