package com.bong.client.cultivation.voidaction;

import com.bong.client.BongClient;
import com.bong.client.ui.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;

public final class VoidActionScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_void_action_screen";
    private static KeyBinding openScreenKey;

    private VoidActionScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(VoidActionScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered void action screen bootstrap on key: UNKNOWN");
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
            openScreenKey = BongKeybindRegistry.global().register(
                new BongKeybindRegistry.BindingSpec(
                    new BongKeybindRegistry.BindingOwner("void_action.open_screen"),
                    OPEN_KEY_TRANSLATION,
                    InputUtil.Type.KEYSYM,
                    InputUtil.UNKNOWN_KEY.getCode(),
                    CATEGORY
                )
            );
        }
        return openScreenKey;
    }
}
