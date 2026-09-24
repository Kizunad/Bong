package com.bong.client.animation;

import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.WeaponEquippedStore;
import com.bong.client.movement.MovementState;
import com.bong.client.movement.MovementStateStore;
import dev.kosmx.playerAnim.api.layered.AnimationStack;
import dev.kosmx.playerAnim.minecraftApi.PlayerAnimationAccess;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Vec3d;

import java.util.Objects;
import java.util.UUID;

/**
 * 下半身步态驱动：每 client tick 选档，把对应动画挂到 {@link AnimationLayerManager.Channel#LOWER_BODY}。
 *
 * <p>上下分离的下半边。四条全局步态（{@code lower_walk/jog/sprint/dash}）只写
 * leftLeg/rightLeg/body，不碰 arm/torso/head；PlayerAnimator 对没写关键帧的 axis 是
 * 原样透传（{@code KeyframeAnimationPlayer.Axis.getValueAtCurrentTick} 尾部
 * {@code return currentValue}），所以上半身照常由招式动画或 vanilla 接管。
 *
 * <p>{@link GaitVariants} 的**携行变体**是这条契约的一处**有意放宽**：它额外写双臂
 * （持刀走路该有持刀的手型），但仍不写 torso/head。本通道 priority 500 低于
 * {@code UPPER_BODY} 的 1000，施法/攻击时手臂由上层接管，不会打架；没有招式在播时
 * 才看得到携行手型。
 *
 * <p>档位判定见 {@link GaitSelector}（纯函数，单测覆盖）；手持物驱动的**变体**解析见
 * {@link GaitVariants}。换动画的判据是**解析后的 animId**而不是档位——同一个 WALK 档，
 * 手里换了把采药刀就该切到携行变体，只比档位会漏掉这次切换。
 */
public final class LowerBodyGaitController {
    /** dash 是一次性动画，播完自己结束；期间不因档位重算而被打断。 */
    private static GaitSelector.Gait activeGait = GaitSelector.Gait.NONE;
    /** 当前实际在播的动画 id（含变体）。档位没变但手持物变了时靠它发现要换。 */
    private static Identifier activeAnimId;
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
        apply(player, next, heldTemplateId());
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

    /** 主手 Bong 物品的 template_id；空手 / 非 Bong 物品返回 null。 */
    static String heldTemplateId() {
        EquippedWeapon weapon = WeaponEquippedStore.mainHandRenderWeapon();
        return weapon == null ? null : weapon.templateId();
    }

    static void apply(AbstractClientPlayerEntity player, GaitSelector.Gait next) {
        apply(player, next, null);
    }

    static void apply(AbstractClientPlayerEntity player, GaitSelector.Gait next,
                      String heldTemplateId) {
        if (player == null || next == null) {
            return;
        }
        applyOnStack(PlayerAnimationAccess.getPlayerAnimLayer(player), player.getUuid(), next,
            heldTemplateId);
    }

    static void applyOnStack(AnimationStack stack, UUID playerId, GaitSelector.Gait next) {
        applyOnStack(stack, playerId, next, null);
    }

    static void applyOnStack(AnimationStack stack, UUID playerId, GaitSelector.Gait next,
                             String heldTemplateId) {
        if (stack == null || playerId == null || next == null) {
            return;
        }
        Identifier animId = GaitVariants.resolve(next, heldTemplateId);
        boolean sameOwner = activePlayerId != null
            && activePlayerId.equals(playerId)
            && activeStack == stack;
        // 比 animId 不比档位：WALK→WALK 但手里换了刀，动画也得换
        if (sameOwner && next == activeGait && Objects.equals(animId, activeAnimId)) {
            return;
        }
        if (animId == null) {
            if (stopActive()) {
                clearActive();
            }
            return;
        }
        boolean played = AnimationLayerManager.playOnStack(
            stack,
            playerId,
            AnimationLayerManager.Channel.LOWER_BODY,
            animId,
            next.fadeInTicks(),
            sameOwner ? activeGait.fadeOutTicks() : 0
        );

        if (!played) {
            if (AnimationLayerManager.activeInChannel(
                playerId, AnimationLayerManager.Channel.LOWER_BODY
            ) == null) {
                clearActive();
            }
            return;
        }

        activeGait = next;
        activeAnimId = animId;
        activePlayerId = playerId;
        activeStack = stack;
    }

    private static boolean stopActive() {
        if (activePlayerId == null || activeStack == null || activeGait == GaitSelector.Gait.NONE) {
            return true;
        }
        return AnimationLayerManager.stopOnStack(
            activeStack,
            activePlayerId,
            AnimationLayerManager.Channel.LOWER_BODY,
            activeGait.fadeOutTicks()
        );
    }

    private static void clearActive() {
        activeGait = GaitSelector.Gait.NONE;
        activeAnimId = null;
        activePlayerId = null;
        activeStack = null;
    }

    public static void clearOnDisconnect() {
        clearActive();
    }

    static void resetForTests() {
        clearOnDisconnect();
    }

    static GaitSelector.Gait activeGaitForTests() {
        return activeGait;
    }

    static Identifier activeAnimIdForTests() {
        return activeAnimId;
    }
}
