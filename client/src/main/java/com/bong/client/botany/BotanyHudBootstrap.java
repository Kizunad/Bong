package com.bong.client.botany;

import com.bong.client.BongClient;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.input.Input;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class BotanyHudBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String AUTO_KEY_TRANSLATION = "key.bong-client.botany_auto_harvest";
    private static KeyBinding autoHarvestKey;

    private BotanyHudBootstrap() {
    }

    public static void register() {
        autoHarvestKey();
        ClientTickEvents.START_CLIENT_TICK.register(BotanyHudBootstrap::onStartClientTick);
        ClientTickEvents.END_CLIENT_TICK.register(BotanyHudBootstrap::onEndClientTick);
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) -> client.execute(BotanyHudBootstrap::resetOnDisconnect));
        BongClient.LOGGER.info("Botany HUD bootstrap ready: manual via inventory key, auto via R.");
    }

    public static boolean shouldCaptureSpellVolumeKey() {
        return HarvestSessionStore.capturesReservedInput();
    }

    static void resetOnDisconnect() {
        HarvestSessionStore.clearOnDisconnect();
        BotanySkillStore.clearOnDisconnect();
    }

    private static void onStartClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }
        HarvestSessionViewModel session = HarvestSessionStore.snapshot();
        if (!session.interactive() || client.currentScreen != null) {
            return;
        }

        if (consumeManualPress(client)) {
            dispatchModeRequest(session, BotanyHarvestMode.MANUAL);
        }

        while (autoHarvestKey().wasPressed()) {
            dispatchModeRequest(session, BotanyHarvestMode.AUTO);
        }
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }

        // 拖拽跟踪：即使 session 未选模式也要消费鼠标移动，保持 panel 平滑跟随
        if (BotanyDragState.isDragging()) {
            double mx = client.mouse.getX() * client.getWindow().getScaledWidth()
                / (double) client.getWindow().getWidth();
            double my = client.mouse.getY() * client.getWindow().getScaledHeight()
                / (double) client.getWindow().getHeight();
            BotanyDragState.tickDrag(mx, my);
        }

        HarvestSessionViewModel session = HarvestSessionStore.snapshot();
        if (!session.interactive() || session.mode() == null) {
            return;
        }

        long nowMillis = System.currentTimeMillis();
        if (client.player.hurtTime > 0) {
            HarvestSessionStore.interruptLocally("受击打断", nowMillis);
            return;
        }

        if (session.mode() == BotanyHarvestMode.MANUAL && isMoving(client)) {
            HarvestSessionStore.interruptLocally("移动打断", nowMillis);
        }
    }

    private static KeyBinding autoHarvestKey() {
        if (autoHarvestKey == null) {
            autoHarvestKey = KeyBindingHelper.registerKeyBinding(
                new KeyBinding(AUTO_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_R, CATEGORY)
            );
        }
        return autoHarvestKey;
    }

    private static boolean consumeManualPress(MinecraftClient client) {
        boolean pressed = false;
        while (client.options.inventoryKey.wasPressed()) {
            pressed = true;
        }
        if (pressed) {
            client.options.inventoryKey.setPressed(false);
        }
        return pressed;
    }

    private static void dispatchModeRequest(HarvestSessionViewModel session, BotanyHarvestMode mode) {
        BotanySkillViewModel skill = BotanySkillStore.snapshot();
        if (!session.interactive() || session.sessionId().isEmpty() || session.requestPending()) {
            return;
        }
        if (mode == BotanyHarvestMode.AUTO && (!session.autoSelectable() || !skill.autoUnlocked())) {
            return;
        }
        HarvestSessionStore.requestMode(mode, System.currentTimeMillis());
        ClientRequestSender.sendBotanyHarvestRequest(session.sessionId(), mode);
    }

    private static boolean isMoving(MinecraftClient client) {
        Input input = client.player.input;
        return input != null && (
            input.pressingForward
                || input.pressingBack
                || input.pressingLeft
                || input.pressingRight
                || input.jumping
        );
    }
}
