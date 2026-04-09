package com.bong.client.ui;

import com.bong.client.BongClient;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class CultivationScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_cultivation_screen";
    private static KeyBinding openScreenKey;

    private CultivationScreenBootstrap() {
    }

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(CultivationScreenBootstrap::onEndClientTick);
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) ->
            client.execute(CultivationScreenBootstrap::clearPlayerStateSnapshot)
        );
        BongClient.LOGGER.info("Registered cultivation screen bootstrap keybinding on key: K");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }

        while (consumeClick(keyBinding())) {
            requestOpenCultivationScreen(client);
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = KeyBindingHelper.registerKeyBinding(
                new KeyBinding(OPEN_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_K, CATEGORY)
            );
        }

        return openScreenKey;
    }

    private static boolean consumeClick(KeyBinding keyBinding) {
        return keyBinding.wasPressed();
    }

    private static void requestOpenCultivationScreen(MinecraftClient client) {
        client.execute(() -> client.setScreen(createScreenForCurrentState()));
    }

    static void clearPlayerStateSnapshot() {
        PlayerStateStore.replace(null);
    }

    static CultivationScreen createScreenForCurrentState() {
        return createScreen(PlayerStateStore.snapshot());
    }

    static CultivationScreen createScreen(PlayerStateViewModel playerState) {
        return new CultivationScreen(playerState);
    }
}
