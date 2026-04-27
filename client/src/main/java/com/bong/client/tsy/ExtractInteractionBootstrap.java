package com.bong.client.tsy;

import com.bong.client.BongClient;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;

public final class ExtractInteractionBootstrap {
    private static boolean useHeldLastTick;
    private static boolean escapeHeldLastTick;

    private ExtractInteractionBootstrap() {
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(ExtractInteractionBootstrap::onTick);
        BongClient.LOGGER.info("Registered TSY extract interaction polling (USE to start, ESC to cancel).");
    }

    private static void onTick(MinecraftClient client) {
        if (client == null || client.player == null || client.options == null) {
            useHeldLastTick = false;
            escapeHeldLastTick = false;
            return;
        }
        boolean useHeld = client.options.useKey.isPressed();
        if (useHeld && !useHeldLastTick && !ExtractStateStore.snapshot().extracting()) {
            RiftPortalView portal = ExtractStateStore.nearestPortal(client.player);
            if (portal != null) {
                ClientRequestSender.sendStartExtract(portal.entityId());
            }
        }
        useHeldLastTick = useHeld;

        boolean escapeHeld = org.lwjgl.glfw.GLFW.glfwGetKey(
            client.getWindow().getHandle(),
            org.lwjgl.glfw.GLFW.GLFW_KEY_ESCAPE
        ) == org.lwjgl.glfw.GLFW.GLFW_PRESS;
        if (escapeHeld && !escapeHeldLastTick && ExtractStateStore.snapshot().extracting()) {
            ClientRequestSender.sendCancelExtract();
        }
        escapeHeldLastTick = escapeHeld;
    }
}
