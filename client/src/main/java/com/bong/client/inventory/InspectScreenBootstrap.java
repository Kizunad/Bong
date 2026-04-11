package com.bong.client.inventory;

import com.bong.client.BongClient;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.MockInventoryData;
import com.bong.client.inventory.state.InventoryStateStore;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class InspectScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_inspect_screen";
    private static KeyBinding openScreenKey;

    private InspectScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(InspectScreenBootstrap::onEndClientTick);
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) ->
            client.execute(() -> InventoryStateStore.replace(null))
        );
        BongClient.LOGGER.info("Registered inspect screen bootstrap keybinding on key: I");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        while (keyBinding().wasPressed()) {
            requestOpenInspectScreen(client);
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = KeyBindingHelper.registerKeyBinding(
                new KeyBinding(OPEN_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_I, CATEGORY)
            );
        }
        return openScreenKey;
    }

    private static void requestOpenInspectScreen(MinecraftClient client) {
        client.execute(() -> {
            if (client.currentScreen instanceof InspectScreen) return;
            client.setScreen(createScreenForCurrentState());
        });
    }

    static InspectScreen createScreenForCurrentState() {
        InventoryModel snapshot = InventoryStateStore.snapshot();
        if (snapshot.isEmpty()) {
            snapshot = MockInventoryData.create();
        }
        return new InspectScreen(snapshot);
    }
}
