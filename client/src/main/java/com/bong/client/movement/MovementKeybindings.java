package com.bong.client.movement;

import com.bong.client.BongClient;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;

public final class MovementKeybindings {
    private static final MovementKeyRouter ROUTER = new MovementKeyRouter();
    private static boolean registered;

    private MovementKeybindings() {
    }

    public static void register() {
        if (registered) {
            return;
        }
        ClientTickEvents.END_CLIENT_TICK.register(MovementKeybindings::onEndClientTick);
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) -> client.execute(MovementKeybindings::resetOnDisconnect));
        registered = true;
        BongClient.LOGGER.info("Movement key router ready: W double-tap/Shift+W dash, Ctrl slide, airborne Space double jump.");
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null || client.currentScreen != null) {
            return;
        }

        boolean forwardTapped = consumeWasPressed(client.options.forwardKey);
        boolean sneakTapped = consumeWasPressed(client.options.sneakKey);
        boolean sprintTapped = consumeWasPressed(client.options.sprintKey);
        boolean jumpTapped = consumeWasPressed(client.options.jumpKey);

        ClientRequestProtocol.MovementAction action = ROUTER.route(
            client.options.forwardKey.isPressed(),
            forwardTapped,
            sneakTapped,
            sprintTapped,
            jumpTapped,
            !client.player.isOnGround(),
            System.currentTimeMillis()
        );
        if (action != null) {
            ClientRequestSender.sendMovementAction(action);
        }
    }

    private static boolean consumeWasPressed(KeyBinding key) {
        boolean pressed = false;
        while (key != null && key.wasPressed()) {
            pressed = true;
        }
        return pressed;
    }

    static void resetOnDisconnect() {
        ROUTER.reset();
        MovementStateStore.clear();
    }
}
