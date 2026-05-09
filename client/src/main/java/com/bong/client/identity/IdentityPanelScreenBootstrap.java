package com.bong.client.identity;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

/** plan-identity-v1 P5：按 O 打开身份面板；server 侧继续校验灵龛与冷却。 */
public final class IdentityPanelScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_identity_panel";
    static final int DEFAULT_KEY = GLFW.GLFW_KEY_O;

    private static KeyBinding openScreenKey;

    private IdentityPanelScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(IdentityPanelScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered identity panel bootstrap keybinding on key: O");
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
            if (client.currentScreen instanceof IdentityPanelScreen) {
                return;
            }
            client.setScreen(new IdentityPanelScreen());
        });
    }
}
