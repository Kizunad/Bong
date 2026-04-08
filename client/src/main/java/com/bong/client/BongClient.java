package com.bong.client;

import com.bong.client.ui.CultivationScreen;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.lwjgl.glfw.GLFW;

public class BongClient implements ClientModInitializer {
    public static final Logger LOGGER = LoggerFactory.getLogger("bong-client");
    private static final String OPEN_CULTIVATION_KEY = "key.bong.open_cultivation";
    private static final String KEY_CATEGORY = "key.categories.bong";

    @Override
    public void onInitializeClient() {
        LOGGER.info("Initializing Bong Client...");

        com.bong.client.network.BongNetworkHandler.register();
        HudRenderCallback.EVENT.register(BongHud::render);
        registerCultivationKeybinding();
    }

    private static void registerCultivationKeybinding() {
        KeyBinding openCultivationKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(OPEN_CULTIVATION_KEY, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_K, KEY_CATEGORY)
        );

        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            while (openCultivationKey.wasPressed()) {
                if (client.player == null || client.world == null) {
                    continue;
                }
                if (client.currentScreen instanceof CultivationScreen) {
                    continue;
                }

                client.setScreen(new CultivationScreen(PlayerStateCache.peek()));
            }
        });
    }
}
