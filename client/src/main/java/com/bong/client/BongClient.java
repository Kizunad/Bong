package com.bong.client;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.lwjgl.glfw.GLFW;

public class BongClient implements ClientModInitializer {
    public static final Logger LOGGER = LoggerFactory.getLogger("bong-client");
    static final String CULTIVATION_SCREEN_KEY_TRANSLATION_KEY = "key.bong-client.open_cultivation_screen";
    static final String CULTIVATION_SCREEN_CATEGORY_TRANSLATION_KEY = "category.bong-client.ui";

    private static KeyBinding cultivationScreenKey;

    @Override
    public void onInitializeClient() {
        LOGGER.info("Initializing Bong Client...");

        BongNetworkHandler.register();
        HudRenderCallback.EVENT.register(BongHud::render);
        cultivationScreenKey = KeyBindingHelper.registerKeyBinding(new KeyBinding(
                CULTIVATION_SCREEN_KEY_TRANSLATION_KEY,
                InputUtil.Type.KEYSYM,
                GLFW.GLFW_KEY_K,
                CULTIVATION_SCREEN_CATEGORY_TRANSLATION_KEY
        ));
        ClientTickEvents.END_CLIENT_TICK.register(BongClient::handleClientTick);
    }

    static void handleClientTick(MinecraftClient client) {
        if (client == null || cultivationScreenKey == null) {
            return;
        }

        while (cultivationScreenKey.wasPressed()) {
            if (shouldOpenCultivationScreen(true, client.currentScreen)) {
                client.setScreen(new CultivationScreen());
            }
        }
    }

    static boolean shouldOpenCultivationScreen(boolean keyPressTriggered, Object currentScreen) {
        return keyPressTriggered && !(currentScreen instanceof CultivationScreen);
    }
}
