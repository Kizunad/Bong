package com.bong.client.spirittreasure;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class SpiritTreasureScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_spirit_treasure_screen";
    static final int DEFAULT_KEY = GLFW.GLFW_KEY_T;

    private static KeyBinding openScreenKey;

    private SpiritTreasureScreenBootstrap() {
    }

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(SpiritTreasureScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered spirit treasure screen bootstrap keybinding on key: T");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }
        while (keyBinding().wasPressed()) {
            requestOpenScreen(client);
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = KeyBindingHelper.registerKeyBinding(
                new KeyBinding(OPEN_KEY_TRANSLATION, InputUtil.Type.KEYSYM, DEFAULT_KEY, CATEGORY)
            );
        }
        return openScreenKey;
    }

    private static void requestOpenScreen(MinecraftClient client) {
        client.execute(() -> {
            if (client.currentScreen instanceof SpiritTreasureScreen) {
                return;
            }
            client.setScreen(new SpiritTreasureScreen());
        });
    }
}
