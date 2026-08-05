package com.bong.client.animation;

import com.bong.client.movement.MovementState;
import com.bong.client.movement.MovementStateStore;
import dev.kosmx.playerAnim.api.layered.AnimationStack;
import dev.kosmx.playerAnim.minecraftApi.PlayerAnimationAccess;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Vec3d;

import java.util.UUID;

/**
 * 下半身步态驱动：每 client tick 选档，把对应动画挂到 {@link AnimationLayerManager.Channel#LOWER_BODY}。
 *
 * <p>上下分离的下半边。四条步态动画（{@code lower_walk/jog/sprint/dash}）只写
 * leftLeg/rightLeg/body，不碰 arm/torso/head；PlayerAnimator 对没写关键帧的 axis 是
 * 原样透传（{@code KeyframeAnimationPlayer.Axis.getValueAtCurrentTick} 尾部
 * {@code return currentValue}），所以上半身照常由招式动画或 vanilla 接管。
 *
 * <p>档位判定见 {@link GaitSelector}（纯函数，单测覆盖）。本类只负责"档位变了才换动画"，
 * 避免每 tick 重复 play 把动画从头顶掉。
 */
public final class LowerBodyGaitController {
    /** dash 是一次性动画，播完自己结束；期间不因档位重算而被打断。 */
    private static GaitSelector.Gait activeGait = GaitSelector.Gait.NONE;
    private static UUID activePlayerId;
    private static AnimationStack activeStack;

    private LowerBodyGaitController() {
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(LowerBodyGaitController::onEndClientTick);
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null || client.isPaused()) {
            return;
        }
        AbstractClientPlayerEntity player = client.player;
        GaitSelector.Gait next = GaitSelector.select(sample(player));
        apply(player, next);
    }

    /** 从 vanilla 玩家 + 服务端 movement_state 采样档位输入。 */
    static GaitSelector.GaitInput sample(AbstractClientPlayerEntity player) {
        MovementState state = MovementStateStore.snapshot();
        Vec3d velocity = player.getVelocity();
        double horizontal = Math.hypot(velocity.x, velocity.z);
        boolean dashing = state != null && state.action() == MovementState.Action.DASHING;
        double multiplier = state == null ? 1.0 : state.currentSpeedMultiplier();
        return new GaitSelector.GaitInput(
            dashing,
            multiplier,
            player.isSprinting(),
            horizontal,
            player.isOnGround()
        );
    }

    static void apply(AbstractClientPlayerEntity player, GaitSelector.Gait next) {
        if (player == null || next == null) {
            return;
        }
        AnimationStack stack = PlayerAnimationAccess.getPlayerAnimLayer(player);
        UUID playerId = player.getUuid();
        boolean sameOwner = activePlayerId != null
            && activePlayerId.equals(playerId)
            && activeStack == stack;
        if (sameOwner && next == activeGait) {
            return;
        }
        Identifier animId = next.animId();
        if (animId == null) {
            AnimationLayerManager.stopOnStack(
                stack,
                playerId,
                AnimationLayerManager.Channel.LOWER_BODY,
                BongAnimationPlayer.DEFAULT_FADE_OUT_TICKS
            );
            activeGait = next;
            activePlayerId = playerId;
            activeStack = stack;
            return;
        }
        boolean played = AnimationLayerManager.playOnStack(
            stack,
            playerId,
            AnimationLayerManager.Channel.LOWER_BODY,
            animId
        );
        if (played) {
            activeGait = next;
            activePlayerId = playerId;
            activeStack = stack;
        }
    }

    public static void clearOnDisconnect() {
        activeGait = GaitSelector.Gait.NONE;
        activePlayerId = null;
        activeStack = null;
    }

    static void resetForTests() {
        clearOnDisconnect();
    }

    static GaitSelector.Gait activeGaitForTests() {
        return activeGait;
    }
}
