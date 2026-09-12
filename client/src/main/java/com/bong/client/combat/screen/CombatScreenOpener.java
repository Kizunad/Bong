package com.bong.client.combat.screen;

import com.bong.client.combat.DeathScreenBootstrap;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.combat.store.TerminateStateStore;
import com.bong.client.ui.ScreenTransitionController;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

import java.util.function.Consumer;

/** 根据服务端快照统一管理死亡／终结屏的开关，包括尚未完成的转场。 */
public final class CombatScreenOpener {
    private CombatScreenOpener() {}

    public static void tick() {
        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc == null) return;
        tick(mc.currentScreen, ScreenTransitionController.activeTransition(), mc::setScreen);
    }

    static void tick(
        Screen current,
        ScreenTransitionController.ActiveTransition transition,
        Consumer<Screen> setScreen
    ) {
        // 转场期间 current 仍是旧屏；以目标屏判重，关屏目标 null 也必须保留。
        Screen target = transition == null ? current : transition.handle().newScreen();

        TerminateStateStore.State term = TerminateStateStore.snapshot();
        if (term.visible()) {
            if (!(target instanceof TerminateScreen)) {
                setScreen.accept(new TerminateScreen(term));
            }
            return;
        }

        DeathStateStore.State death = DeathStateStore.snapshot();
        if (death.visible()) {
            if (!(target instanceof DeathScreen)) {
                setScreen.accept(DeathScreenBootstrap.create(death));
            }
        } else if (target instanceof DeathScreen || target instanceof TerminateScreen) {
            setScreen.accept(null);
        }
    }
}
