package com.bong.client.movement;

import com.bong.client.BongClient;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.input.Input;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class MovementKeybindings {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String DASH_KEY_TRANSLATION = "key.bong-client.movement_dash";
    private static final MovementKeyRouter ROUTER = new MovementKeyRouter();
    private static boolean registered;
    private static KeyBinding dashKey;

    private MovementKeybindings() {
    }

    public static void register() {
        if (registered) {
            return;
        }
        dashKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(DASH_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_V, CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(MovementKeybindings::onEndClientTick);
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) -> client.execute(MovementKeybindings::resetOnDisconnect));
        registered = true;
        BongClient.LOGGER.info("Movement key router ready: configurable dash key.");
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null || client.currentScreen != null) {
            return;
        }

        boolean dashTapped = consumeWasPressed(dashKey);

        ClientRequestProtocol.MovementAction action = ROUTER.route(
            dashTapped
        );
        if (action != null) {
            ClientRequestSender.sendMovementAction(action, dashYawDegrees(client));
        }
    }

    static double dashYawDegrees(MinecraftClient client) {
        double playerYaw = client.player.getYaw();
        Input input = client.player.input;
        double forward = input == null ? 0.0 : input.movementForward;
        double sideways = input == null ? 0.0 : input.movementSideways;
        return resolveDashYawDegrees(playerYaw, forward, sideways);
    }

    /**
     * 合成 dash 朝向：以玩家体朝向 yaw 为参考帧，叠加 WASD 输入角偏移。
     *
     * <p>dash 是纯水平位移，「朝向 + 方向键」折叠成单一 effective yaw，
     * 直接走现有 {@code yaw_degrees} 协议字段，服务端不需改动。</p>
     *
     * <p>参考帧用<b>玩家 yaw</b>（非相机 yaw），使第一人称 / 第三人称背视 / 第三人称
     * 正面视三种视角行为一致——WASD 始终相对角色体朝向（与 MC 原版走路同帧），
     * 正面视下不会因相机偏转 180° 而反向。无方向键时即为朝体朝向正前 dash。</p>
     *
     * @param movementForward  W=+1 / S=-1（可被潜行等缩放，仅符号与比例参与定向）
     * @param movementSideways A=+1（左）/ D=-1（右）
     */
    static double resolveDashYawDegrees(
        double playerYawDegrees,
        double movementForward,
        double movementSideways
    ) {
        return playerYawDegrees + wasdYawOffsetDegrees(movementForward, movementSideways);
    }

    /**
     * 把 WASD 输入折算成相对「正前」的偏航角（度）。
     *
     * <p>对齐 MC 运动帧：本地输入向量 {@code (sideways, forward)} 的朝向角 =
     * {@code atan2(-sideways, forward)}。叠加到参考 yaw 后，经服务端
     * {@code horizontal_direction_from_yaw} 的 {@code (-sin, cos)} 映射得到正确世界方向。</p>
     *
     * <ul>
     *   <li>W → 0°（正前）, S → 180°（正后）</li>
     *   <li>A → -90°（左）, D → +90°（右）</li>
     *   <li>A+S → -135°（后偏左 45°）, D+S → +135°（后偏右 45°）</li>
     * </ul>
     *
     * <p>无方向键（forward≈sideways≈0）→ 0 偏移，保持朝视线正前 dash。</p>
     */
    static double wasdYawOffsetDegrees(double movementForward, double movementSideways) {
        if (Math.abs(movementForward) < 1e-4 && Math.abs(movementSideways) < 1e-4) {
            return 0.0;
        }
        return Math.toDegrees(Math.atan2(-movementSideways, movementForward));
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
