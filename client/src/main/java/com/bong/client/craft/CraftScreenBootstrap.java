package com.bong.client.craft;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

/** plan-craft-ux-v1 — C 键打开手搓台。 */
public final class CraftScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_craft_screen";
    private static KeyBinding openScreenKey;

    private CraftScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(CraftScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered craft screen bootstrap keybinding on key: C");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }
        while (keyBinding().wasPressed()) {
            client.execute(() -> {
                if (!(client.currentScreen instanceof CraftScreen)) {
                    client.setScreen(new CraftScreen());
                }
            });
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = KeyBindingHelper.registerKeyBinding(
                new KeyBinding(OPEN_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_C, CATEGORY)
            );
        }
        return openScreenKey;
    }
}
