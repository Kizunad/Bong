package com.bong.client.cultivation.voidaction;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class VoidActionScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_void_action_screen";
    private static KeyBinding openScreenKey;

    private VoidActionScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(VoidActionScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered void action screen bootstrap on key: O");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        while (keyBinding().wasPressed()) {
            client.execute(() -> {
                if (!(client.currentScreen instanceof VoidActionScreen)) {
                    client.setScreen(new VoidActionScreen());
                }
            });
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = KeyBindingHelper.registerKeyBinding(
                new KeyBinding(OPEN_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_O, CATEGORY)
            );
        }
        return openScreenKey;
    }
}
