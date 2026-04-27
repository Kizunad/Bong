package com.bong.client.tsy;

import com.bong.client.BongClient;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class ExtractInteractionBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String EXTRACT_KEY_TRANSLATION = "key.bong-client.tsy_extract";
    private static final String CANCEL_KEY_TRANSLATION = "key.bong-client.tsy_extract_cancel";
    private static KeyBinding extractKey;
    private static KeyBinding cancelKey;

    private ExtractInteractionBootstrap() {
    }

    public static void register() {
        extractKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(EXTRACT_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_Y, CATEGORY)
        );
        cancelKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(CANCEL_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_U, CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(ExtractInteractionBootstrap::onTick);
        BongClient.LOGGER.info("Registered TSY extract keybindings on keys: Y/U");
    }

    private static void onTick(MinecraftClient client) {
        if (client == null || client.player == null || client.options == null) {
            return;
        }
        while (extractKey.wasPressed() && !ExtractStateStore.snapshot().extracting()) {
            RiftPortalView portal = ExtractStateStore.nearestPortal(client.player);
            if (portal != null) {
                ClientRequestSender.sendStartExtract(portal.entityId());
            }
        }

        while (cancelKey.wasPressed() && ExtractStateStore.snapshot().extracting()) {
            ClientRequestSender.sendCancelExtract();
        }
    }
}
