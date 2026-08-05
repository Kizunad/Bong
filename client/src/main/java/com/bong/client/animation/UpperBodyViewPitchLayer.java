package com.bong.client.animation;

import dev.kosmx.playerAnim.api.TransformType;
import dev.kosmx.playerAnim.api.layered.IAnimation;
import dev.kosmx.playerAnim.minecraftApi.PlayerAnimationAccess;
import dev.kosmx.playerAnim.core.util.Vec3f;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.util.math.MathHelper;
import org.jetbrains.annotations.NotNull;

import java.util.function.BooleanSupplier;
import java.util.function.DoubleSupplier;

/**
 * 上半身跟随视角倾角——程序化动画层（不是关键帧动画）。
 *
 * <p>为什么必须是 procedural：值由 {@code player.getPitch()} 连续驱动，关键帧表达不了。
 * PlayerAnimator 的 {@link dev.kosmx.playerAnim.api.layered.AnimationStack} 是链式透传
 * （{@code value0 = layer.get3DTransform(..., value0)}，低 priority 先算），任何
 * {@link IAnimation} 都能插进去，只改自己关心的 part。
 *
 * <p><b>用 torso.bend 而不是 torso.pitch</b>：torso.pitch 把整个躯干绕腰转、腿完全不跟，
 * 读作"腰断"；torso.bend 是腰部真折弯（bendy-lib 把 cuboid 从几何中心切开转一半），
 * 上半身独立俯仰而胯保持不动，正是上下分离要的效果。实测对比见
 * {@code scripts/models/render_bend_matrix.png}。
 *
 * <p><b>叠加而非覆盖</b>：返回 {@code value0 + 自己的量}。下层（下半身步态层）不写 torso，
 * 所以常态就是纯本层；上层招式动画 priority 更高、后算，写了 torso.bend 就自然接管。
 *
 * <p>分档（用户 2026-08-06 决定"始终生效但分档"）：常态只跟 {@link #CASUAL_MAX_DEG}
 * 的小幅度（含胸/挺背级别），持械或战斗时放开到 {@link #ARMED_MAX_DEG}。
 *
 * <p>平滑：{@link #tick()} 里向目标逼近（每 tick 走 {@link #SMOOTH_RATE} 的差值），
 * 渲染时按 tickDelta 在上一 tick 与当前值之间插值——否则甩视角会让躯干抽搐。
 */
public final class UpperBodyViewPitchLayer implements IAnimation {
    /** 常态最大折弯（度）——日常走路只是含胸/挺背的程度。 */
    public static final float CASUAL_MAX_DEG = 15.0f;
    /** 持械/战斗态最大折弯（度）。 */
    public static final float ARMED_MAX_DEG = 40.0f;
    /** 每 tick 向目标逼近的比例（0-1）。0.35 ≈ 3 tick 到位，甩视角不抽。 */
    public static final float SMOOTH_RATE = 0.35f;
    /** bend 方向：绕 +X 轴折 = 在 YZ 平面前后弯腰。 */
    public static final float BEND_AXIS_RAD = 0.0f;
    /** 本层 priority：高于下半身步态(500)、低于上半身招式(1000)，招式一播就接管 torso。 */
    public static final int PRIORITY = 700;

    /** 视角 pitch 来源（度）。抽成注入点：单测直接喂角度，不必起 MC 运行时。 */
    private final DoubleSupplier viewPitchDeg;
    private final BooleanSupplier armed;

    private float previousDeg;
    private float currentDeg;

    public UpperBodyViewPitchLayer(DoubleSupplier viewPitchDeg, BooleanSupplier armed) {
        this.viewPitchDeg = viewPitchDeg == null ? () -> 0.0 : viewPitchDeg;
        this.armed = armed == null ? () -> false : armed;
    }

    /** 生产构造：跟随该玩家的视角。 */
    public static UpperBodyViewPitchLayer forPlayer(AbstractClientPlayerEntity player, BooleanSupplier armed) {
        return new UpperBodyViewPitchLayer(player == null ? null : player::getPitch, armed);
    }

    /**
     * 每玩家挂一层。PlayerAnimator 在玩家实体初始化时触发该事件，layer 随玩家生命周期存在；
     * {@link #tick()} 由 {@code AnimationStack.tick()} 驱动，不需要自己注册 tick 回调。
     *
     * <p>持械判定：主手非空即算持械（双锏/剑等）。真正的"Bong 武器"判定等武器注册表统一
     * 出口后再收窄，此处先用可注入的谓词占位，不把耦合写死。
     */
    public static void register() {
        PlayerAnimationAccess.REGISTER_ANIMATION_EVENT.register((player, stack) ->
            stack.addAnimLayer(PRIORITY, forPlayer(player, () -> !player.getMainHandStack().isEmpty()))
        );
    }

    /** 目标折弯角（度）：视角 pitch 归一化到 [-1,1] 后乘当前档位上限。 */
    float targetDeg() {
        float normalized = MathHelper.clamp((float) viewPitchDeg.getAsDouble() / 90.0f, -1.0f, 1.0f);
        return normalized * (armed.getAsBoolean() ? ARMED_MAX_DEG : CASUAL_MAX_DEG);
    }

    @Override
    public boolean isActive() {
        return true;
    }

    @Override
    public void tick() {
        previousDeg = currentDeg;
        currentDeg += (targetDeg() - currentDeg) * SMOOTH_RATE;
    }

    @Override
    public void setupAnim(float tickDelta) {
    }

    /** 渲染插值后的折弯量（弧度）。 */
    float bendRadians(float tickDelta) {
        float t = MathHelper.clamp(tickDelta, 0.0f, 1.0f);
        return (float) Math.toRadians(MathHelper.lerp(t, previousDeg, currentDeg));
    }

    @Override
    public @NotNull Vec3f get3DTransform(
        @NotNull String modelName,
        @NotNull TransformType type,
        float tickDelta,
        @NotNull Vec3f value0
    ) {
        if (type != TransformType.BEND || !"torso".equals(modelName)) {
            return value0;
        }
        float bend = bendRadians(tickDelta);
        if (Math.abs(bend) < 1.0e-4f) {
            return value0;   // 没有可见折弯就完全透传，不抢 bendAxis
        }
        // BEND 的 Vec3f 语义 = (bendAxis, bendValue, 0)，与 KeyframeAnimationPlayer 一致
        float axis = Math.abs(value0.getY()) > 1.0e-4f ? value0.getX() : BEND_AXIS_RAD;
        return new Vec3f(axis, value0.getY() + bend, 0.0f);
    }

    float currentDegForTest() {
        return currentDeg;
    }
}
