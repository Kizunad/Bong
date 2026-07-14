package com.bong.client.hud;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.util.function.BooleanSupplier;
import java.util.function.LongSupplier;
import java.util.function.UnaryOperator;

public final class HudImmersionControls {
    private static final String CATEGORY = "category.bong-client";
    private static final String TOGGLE_KEY = "key.bong-client.hud_immersive_toggle";
    private static KeyBinding toggleKey;

    private HudImmersionControls() {
    }

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(HudImmersionControls::onEndClientTick);
    }

    private static void onEndClientTick(MinecraftClient client) {
        consumeTogglePresses(keyBinding()::wasPressed, System::currentTimeMillis);
    }

    static int consumeTogglePresses(BooleanSupplier wasPressed, LongSupplier nowMillis) {
        int consumed = 0;
        while (wasPressed.getAsBoolean()) {
            HudImmersionMode.toggleManual(nowMillis.getAsLong());
            consumed++;
        }
        return consumed;
    }

    private static KeyBinding keyBinding() {
        if (toggleKey == null) {
            toggleKey = registerToggleKey(KeyBindingHelper::registerKeyBinding);
        }
        return toggleKey;
    }

    static KeyBinding registerToggleKey(UnaryOperator<KeyBinding> registrar) {
        // Leave unbound so F1-F9 remain reserved for quick slots.
        return registrar.apply(
            new KeyBinding(TOGGLE_KEY, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_UNKNOWN, CATEGORY)
        );
    }
}
