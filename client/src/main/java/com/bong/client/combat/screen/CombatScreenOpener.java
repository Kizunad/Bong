package com.bong.client.combat.screen;

import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.combat.store.TerminateStateStore;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

/**
 * Polls {@link DeathStateStore} / {@link TerminateStateStore} each frame and
 * opens the corresponding full-screen overlay when the server turns it on.
 * Idempotent; will not replace a screen that is already the right type.
 */
public final class CombatScreenOpener {
    private CombatScreenOpener() {}

    public static void tick() {
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc == null) return;
        Screen current = mc.currentScreen;

        TerminateStateStore.State term = TerminateStateStore.snapshot();
        if (term.visible() && !(current instanceof TerminateScreen)) {
            mc.setScreen(new TerminateScreen(term));
            return;
        }

        DeathStateStore.State death = DeathStateStore.snapshot();
        if (death.visible() && !(current instanceof DeathScreen) && !(current instanceof TerminateScreen)) {
            mc.setScreen(new DeathScreen(death));
        }
    }
}
