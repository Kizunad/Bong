package com.bong.client.mixin;

import com.bong.client.combat.juice.CameraShakeController;
import com.bong.client.hud.BongHudStateStore;
import com.bong.client.state.VisualEffectState;
import com.bong.client.visual.CameraPushbackOffset;
import com.bong.client.visual.CameraShakeOffsets;
import com.bong.client.visual.CameraTiltOffset;
import net.minecraft.client.render.Camera;
import net.minecraft.entity.Entity;
import net.minecraft.world.BlockView;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArgs;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.invoke.arg.Args;

/**
 * 为相机类视觉效果注入旋转与位移偏移——让整个画面真的在动，而不只是文字条。
 *
 * <p>两个注入点：
 * <ul>
 *   <li><b>setRotation 的 ModifyArgs</b>：修改传入的 yaw/pitch，叠加 SCREEN_SHAKE / PRESSURE_JITTER
 *       的抖动和 TRIBULATION_LOOK_UP 的仰视偏移</li>
 *   <li><b>update 的 TAIL Inject</b>：在 vanilla 完成 setPos/moveBy 后，再 {@link Camera#moveBy}
 *       叠加 HIT_PUSHBACK 的后退位移</li>
 * </ul>
 *
 * <p><b>不影响玩家实际朝向与位置</b>：玩家 Entity 的 yaw/pitch/pos 字段不变，
 * 射箭、生物瞄准、方块交互全部按真实状态工作；只是 render 时相机看到的位置/方向加了偏移。
 *
 * <p>非相关 state 时各工具返回零，注入等价于直通。
 */
@Mixin(Camera.class)
public abstract class MixinCamera {

    /**
     * {@link Camera#moveBy} 是 protected，需要 @Shadow 声明才能在 mixin 内调用。
     * Mixin 运行期会把这里的 abstract 方法替换为对真实 {@code moveBy} 的直接调用。
     */
    @Shadow
    protected abstract void moveBy(double x, double y, double z);

    @ModifyArgs(
        method = "update(Lnet/minecraft/world/BlockView;Lnet/minecraft/entity/Entity;ZZF)V",
        at = @At(
            value = "INVOKE",
            target = "Lnet/minecraft/client/render/Camera;setRotation(FF)V"
        )
    )
    private void bong$applyCameraRotationOffsets(Args args) {
        VisualEffectState state = BongHudStateStore.snapshot().visualEffectState();
        long nowMillis = System.currentTimeMillis();
        CameraShakeOffsets.Offsets shake = CameraShakeOffsets.compute(state, nowMillis);
        CameraShakeController.Offsets combatShake = CameraShakeController.activeOffsets(nowMillis);
        float tiltPitch = CameraTiltOffset.computePitchDegrees(state, nowMillis);
        if (shake.isZero() && combatShake.isZero() && tiltPitch == 0f) {
            return;
        }
        float yaw = args.get(0);
        float pitch = args.get(1);
        args.set(0, yaw + shake.yawDegrees() + combatShake.yawDegrees());
        args.set(1, pitch + shake.pitchDegrees() + combatShake.pitchDegrees() + tiltPitch);
    }

    @Inject(
        method = "update(Lnet/minecraft/world/BlockView;Lnet/minecraft/entity/Entity;ZZF)V",
        at = @At("TAIL")
    )
    private void bong$applyHitPushback(
        BlockView area,
        Entity focusedEntity,
        boolean thirdPerson,
        boolean inverseView,
        float tickDelta,
        CallbackInfo ci
    ) {
        VisualEffectState state = BongHudStateStore.snapshot().visualEffectState();
        double distance = CameraPushbackOffset.computeBackwardDistance(state, System.currentTimeMillis());
        if (distance <= 0.0) {
            return;
        }
        // -x 方向 = 远离 facing（和 MC 第三人称拉远同一方向），first/third person 通用
        this.moveBy(-distance, 0.0, 0.0);
    }
}
