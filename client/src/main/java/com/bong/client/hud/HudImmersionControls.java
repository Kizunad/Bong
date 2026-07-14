package com.bong.client.hud;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.util.function.BooleanSupplier;
import java.util.function.LongSupplier;

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
            toggleKey = KeyBindingHelper.registerKeyBinding(
                // plan-bughunt-quick-slot-function-key-collision-v1:
                // F1-F9 are reserved for the visible quick-use row. Keep this
                // convenience toggle discoverable in Controls, but unbound by default.
                new KeyBinding(TOGGLE_KEY, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_UNKNOWN, CATEGORY)
            );
        }
        return toggleKey;
    }
}
