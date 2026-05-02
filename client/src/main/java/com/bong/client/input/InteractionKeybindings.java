package com.bong.client.input;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class InteractionKeybindings {
    public static final String CATEGORY = "category.bong-client.controls";
    public static final String INTERACT_KEY_TRANSLATION = "key.bong-client.interact";
    public static final int DEFAULT_KEY_CODE = GLFW.GLFW_KEY_G;

    private static KeyBinding interactKey;
    private static boolean registered;

    private InteractionKeybindings() {
    }

    public static void register() {
        if (registered) {
            return;
        }
        interactKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(INTERACT_KEY_TRANSLATION, InputUtil.Type.KEYSYM, DEFAULT_KEY_CODE, CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(InteractionKeybindings::onEndClientTick);
        registered = true;
        BongClient.LOGGER.info("Registered unified interaction keybinding on key: G");
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null || client.currentScreen != null) {
            return;
        }
        while (interactKey != null && interactKey.wasPressed()) {
            InteractKeyRouter.global().route(client);
        }
    }

    public static KeyBinding interactKeyForTests() {
        return interactKey;
    }
}
