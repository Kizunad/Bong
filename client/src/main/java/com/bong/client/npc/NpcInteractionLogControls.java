package com.bong.client.npc;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class NpcInteractionLogControls {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String KEY_TRANSLATION = "key.bong-client.npc_interaction_log";
    private static KeyBinding key;
    private static boolean registered;

    private NpcInteractionLogControls() {
    }

    public static void register() {
        if (registered) {
            return;
        }
        key = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_F7, CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(NpcInteractionLogControls::onEndClientTick);
        registered = true;
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null || client.currentScreen != null) {
            return;
        }
        while (key != null && key.wasPressed()) {
            NpcInteractionLogStore.toggleVisible();
        }
    }
}
