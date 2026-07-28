package com.bong.client.npc;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.util.function.BooleanSupplier;
import java.util.function.UnaryOperator;

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
        installInteractionLogKey(KeyBindingHelper::registerKeyBinding);
        ClientTickEvents.END_CLIENT_TICK.register(NpcInteractionLogControls::onEndClientTick);
        registered = true;
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null) {
            return;
        }
        consumeInstalledTogglePresses(
            client.player != null,
            client.currentScreen != null
        );
    }

    static int consumeInstalledTogglePresses(boolean playerPresent, boolean screenOpen) {
        return consumeTogglePresses(
            playerPresent,
            screenOpen,
            () -> key != null && key.wasPressed()
        );
    }

    static int consumeTogglePresses(
        boolean playerPresent,
        boolean screenOpen,
        BooleanSupplier wasPressed
    ) {
        if (!playerPresent || screenOpen) {
            return 0;
        }
        int consumed = 0;
        while (wasPressed.getAsBoolean()) {
            NpcInteractionLogStore.toggleVisible();
            consumed++;
        }
        return consumed;
    }

    static KeyBinding installInteractionLogKey(UnaryOperator<KeyBinding> registrar) {
        // Leave unbound so F1-F9 remain reserved for quick slots.
        key = registrar.apply(
            new KeyBinding(KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_UNKNOWN, CATEGORY)
        );
        return key;
    }

    static void resetControlsForTests() {
        key = null;
        registered = false;
    }
}
