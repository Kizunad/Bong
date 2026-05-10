package com.bong.client.hud;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

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
        while (keyBinding().wasPressed()) {
            HudImmersionMode.toggleManual(System.currentTimeMillis());
        }
    }

    private static KeyBinding keyBinding() {
        if (toggleKey == null) {
            toggleKey = KeyBindingHelper.registerKeyBinding(
                new KeyBinding(TOGGLE_KEY, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_F6, CATEGORY)
            );
        }
        return toggleKey;
    }
}
