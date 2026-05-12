package com.bong.client.movement;

import com.bong.client.BongClient;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class MovementKeybindings {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String DASH_KEY_TRANSLATION = "key.bong-client.movement_dash";
    private static final String SLIDE_KEY_TRANSLATION = "key.bong-client.movement_slide";
    private static final MovementKeyRouter ROUTER = new MovementKeyRouter();
    private static boolean registered;
    private static KeyBinding dashKey;
    private static KeyBinding slideKey;

    private MovementKeybindings() {
    }

    public static void register() {
        if (registered) {
            return;
        }
        dashKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(DASH_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_V, CATEGORY)
        );
        slideKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(SLIDE_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_B, CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(MovementKeybindings::onEndClientTick);
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) -> client.execute(MovementKeybindings::resetOnDisconnect));
        registered = true;
        BongClient.LOGGER.info("Movement key router ready: configurable dash/slide keys, airborne Space double jump.");
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null || client.currentScreen != null) {
            return;
        }

        boolean dashTapped = consumeWasPressed(dashKey);
        boolean slideTapped = consumeWasPressed(slideKey);
        boolean jumpTapped = consumeWasPressed(client.options.jumpKey);

        ClientRequestProtocol.MovementAction action = ROUTER.route(
            dashTapped,
            slideTapped,
            jumpTapped,
            !client.player.isOnGround()
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
