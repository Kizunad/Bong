package com.bong.client.ui;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class CultivationScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_cultivation_screen";
    private static final KeyBinding OPEN_SCREEN_KEY = KeyBindingHelper.registerKeyBinding(
        new KeyBinding(OPEN_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_K, CATEGORY)
    );

    private CultivationScreenBootstrap() {
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(CultivationScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered cultivation screen bootstrap keybinding on key: K");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }

        while (consumeClick(OPEN_SCREEN_KEY)) {
            requestOpenCultivationScreen(client);
        }
    }

    private static boolean consumeClick(KeyBinding keyBinding) {
        return keyBinding.wasPressed();
    }

    private static void requestOpenCultivationScreen(MinecraftClient client) {
        client.execute(() -> BongClient.LOGGER.info("Cultivation screen open requested (skeleton)."));
    }
}
