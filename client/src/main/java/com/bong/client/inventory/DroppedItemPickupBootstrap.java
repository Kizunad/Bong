package com.bong.client.inventory;

import com.bong.client.BongClient;
import com.bong.client.inventory.state.DroppedItemStore;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class DroppedItemPickupBootstrap {
    private static final String CATEGORY = "category.bong-client.inventory";
    private static final String PICKUP_KEY_TRANSLATION = "key.bong-client.pickup_dropped_item";
    private static KeyBinding pickupKey;

    private DroppedItemPickupBootstrap() {}

    public static void register() {
        pickupKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(PICKUP_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_G, CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(DroppedItemPickupBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered dropped loot pickup keybinding on key: G");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        while (pickupKey.wasPressed()) {
            DroppedItemStore.Entry nearest = DroppedItemStore.nearestTo(
                client.player.getX(),
                client.player.getY(),
                client.player.getZ()
            );
            if (nearest == null) {
                return;
            }
            ClientRequestSender.sendPickupDroppedItem(nearest.instanceId());
        }
    }
}
