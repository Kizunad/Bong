package com.bong.client.botany;

import com.bong.client.BongClient;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.input.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.input.Input;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.util.Objects;
import java.util.function.BooleanSupplier;

public final class BotanyHudBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String AUTO_KEY_TRANSLATION = "key.bong-client.botany_auto_harvest";
    /** plan-skill-v1 §6.1：herbalism Lv.3 解锁自动采集。 */
    private static final int HERBALISM_AUTO_UNLOCK_LV = 3;
    private static KeyBinding autoHarvestKey;
    private static boolean registered;

    private BotanyHudBootstrap() {
    }

    public static void register() {
        if (registered) {
            return;
        }
        autoHarvestKey();
        ClientTickEvents.START_CLIENT_TICK.register(BotanyHudBootstrap::onStartClientTick);
        ClientTickEvents.END_CLIENT_TICK.register(BotanyHudBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Botany HUD bootstrap ready: manual via inventory key, auto harvest is configurable.");
        registered = true;
    }

    public static boolean shouldCaptureSpellVolumeKey() {
        return HarvestSessionStore.capturesReservedInput();
    }

    public static void clearOnDisconnect() {
        BotanyDragState.clearOnDisconnect();
    }

    private static void onStartClientTick(MinecraftClient client) {
        if (client == null) {
            return;
        }
        if (client.player == null) {
            discardAutoHarvestPresses();
            return;
        }

        HarvestSessionViewModel session = HarvestSessionStore.snapshot();
        if (!session.interactive() || client.currentScreen != null) {
            discardAutoHarvestPresses();
            return;
        }

        if (consumeManualPress(client)) {
            dispatchModeRequest(session, BotanyHarvestMode.MANUAL);
        }

        pumpAutoHarvestPresses(true, false, autoHarvestKey()::wasPressed);
    }

    /**
     * 消费自动采集按键队列；门控期间也必须取空队列，避免按键跨 tick/会话幽灵重放。
     *
     * <p>每次真正尝试派发前都从 {@link HarvestSessionStore} 读取实时快照。首次派发会把
     * {@code requestPending} 写为 true，后续同 tick 排队按键因此只能被消费，不能再次发包。</p>
     *
     * @return 本次 pump 消费的按键次数（包括门控时丢弃的次数）
     */
    static int pumpAutoHarvestPresses(
        boolean interactive,
        boolean screenOpen,
        BooleanSupplier wasPressed
    ) {
        Objects.requireNonNull(wasPressed, "wasPressed");
        int consumed = 0;
        while (wasPressed.getAsBoolean()) {
            consumed++;
            if (interactive && !screenOpen) {
                dispatchModeRequest(HarvestSessionStore.snapshot(), BotanyHarvestMode.AUTO);
            }
        }
        return consumed;
    }

    private static void discardAutoHarvestPresses() {
        if (autoHarvestKey != null) {
            pumpAutoHarvestPresses(false, true, autoHarvestKey::wasPressed);
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
            autoHarvestKey = BongKeybindRegistry.global().register(
                new BongKeybindRegistry.BindingSpec(
                    new BongKeybindRegistry.BindingOwner("botany.auto_harvest"),
                    AUTO_KEY_TRANSLATION,
                    InputUtil.Type.KEYSYM,
                    InputUtil.UNKNOWN_KEY.getCode(),
                    CATEGORY
                )
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
        if (!session.interactive() || session.sessionId().isEmpty() || session.requestPending()) {
            return;
        }
        if (mode == BotanyHarvestMode.AUTO) {
            int herbalismLv = SkillSetStore.snapshot().get(SkillId.HERBALISM).effectiveLv();
            if (!session.autoSelectable() || herbalismLv < HERBALISM_AUTO_UNLOCK_LV) {
                return;
            }
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
