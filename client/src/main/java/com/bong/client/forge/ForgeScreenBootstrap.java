package com.bong.client.forge;

import com.bong.client.BongClient;
import com.bong.client.ui.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;

/** plan-forge-v1 §3.3 — 右键砧方块 / 按键打开锻炉 UI 的启动器。 */
public final class ForgeScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_forge_screen";
    private static KeyBinding openScreenKey;

    private ForgeScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(ForgeScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info(
            "Registered forge screen bootstrap keybinding (default unbound; configure in controls)."
        );
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        while (keyBinding().wasPressed()) {
            requestOpenForgeScreen(client);
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = BongKeybindRegistry.global().register(
                new BongKeybindRegistry.BindingSpec(
                    new BongKeybindRegistry.BindingOwner("forge.open_screen"),
                    OPEN_KEY_TRANSLATION,
                    InputUtil.Type.KEYSYM,
                    InputUtil.UNKNOWN_KEY.getCode(),
                    CATEGORY
                )
            );
        }
        return openScreenKey;
    }

    private static void requestOpenForgeScreen(MinecraftClient client) {
        client.execute(() -> {
            if (client.currentScreen instanceof ForgeScreen) {
                return;
            }
            client.setScreen(new ForgeScreen());
        });
    }
}
