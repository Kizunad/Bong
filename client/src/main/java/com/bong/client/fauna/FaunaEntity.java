package com.bong.client.fauna;

import net.minecraft.entity.Entity;
import net.minecraft.entity.EntityType;
import net.minecraft.nbt.NbtCompound;
import net.minecraft.world.World;
import software.bernie.geckolib.animatable.GeoEntity;
import software.bernie.geckolib.core.animatable.instance.AnimatableInstanceCache;
import software.bernie.geckolib.core.animation.AnimatableManager;
import software.bernie.geckolib.core.animation.AnimationController;
import software.bernie.geckolib.core.animation.RawAnimation;
import software.bernie.geckolib.core.object.PlayState;
import software.bernie.geckolib.util.GeckoLibUtil;

import java.util.Objects;

public final class FaunaEntity extends Entity implements GeoEntity {
    /** 速度衰减系数：位置包间隙靠它维持 walk/run 不闪回 idle。 */
    private static final float SPEED_DECAY = 0.72f;
    /** 超过此速度（blocks/tick）播 walk。取值极小——任何真实位移都算「在走」。 */
    private static final float WALK_SPEED_THRESHOLD = 0.012f;
    /** 超过此速度播 run。约对应奔跑步速；低于则退回 walk。 */
    private static final float RUN_SPEED_THRESHOLD = 0.14f;
    /** 低于此瞬时位移不更新移动朝向（避免站定时被抖动噪声乱转）。 */
    private static final float FACING_MIN_STEP = 0.004f;
    /** 移动朝向每 tick 最多转多少度（平滑转身，不瞬移）。 */
    private static final float FACING_TURN_DEGREES_PER_TICK = 28.0f;

    private final AnimatableInstanceCache cache = GeckoLibUtil.createInstanceCache(this);
    private final FaunaVisualKind visualKind;

    /**
     * 一次性招式动画状态机（黑武士 boss 出招）。由服务端 {@code play_entity_anim} 经
     * {@code VfxEventRouter} → {@code FaunaActionBridge} → {@link #triggerAction} 驱动。
     */
    private final FaunaActionAnimation action = new FaunaActionAnimation();

    /**
     * 客户端水平移动速度信号（blocks/tick），驱动 idle↔walk↔run 切换。
     *
     * <p>FaunaEntity 是原版 tracker 驱动的 {@link Entity}（非 LivingEntity，无 limbSwing），
     * 位置由服务端位置包逐 tick 塞进来。这里在 {@link #tick()} 里算 (x,z) 逐 tick 位移，
     * 用「瞬时上冲 + 缓慢衰减」平滑：位置包到达那一 tick 速度尖峰、包间隙靠衰减维持，
     * 既不依赖 vanilla 的 lerp 内部实现，也不会因逐包跳变而在 walk/idle 间抖动。
     */
    private double lastTickX;
    private double lastTickZ;
    private boolean hasLastTick;
    private float horizontalSpeed;

    /**
     * 客户端按位移观测到的水平移动朝向（MC yaw，度）。
     *
     * <p>为什么客户端自算：噬元鼠等 fauna 服务端是 {@code MarkerEntityBundle}（minecraft:marker），
     * marker 的旋转 valence 不下发给客户端 → 客户端实体 {@code getYaw()} 恒为 0 → 模型定朝一个
     * 固定方向、不跟移动（"倒着跑"）。这里直接用逐 tick (x,z) 位移反解朝向，绕开 marker 不发 yaw
     * 的限制。仅对 {@link FaunaVisualKind#facesMovementDirection()} 的物种用于渲染朝向。
     */
    private float movementYaw;

    public FaunaEntity(EntityType<? extends FaunaEntity> type, World world, FaunaVisualKind visualKind) {
        super(type, world);
        this.visualKind = Objects.requireNonNull(visualKind, "visualKind");
    }

    public FaunaVisualKind visualKind() {
        return visualKind;
    }

    /**
     * 触发一次性招式动画（由网络层在主线程调用）。{@code durationTicks} tick 后自动回 idle。
     *
     * @param animName      GeckoLib 动画名（如 {@code animation.bong.heiwushi.dark_barrage}）
     * @param durationTicks 动画占用时长（tick）
     * @return {@code true} 成功进入招式态；{@code false} 无效输入被拒（透传 {@link FaunaActionAnimation#trigger}）。
     */
    public boolean triggerAction(String animName, int durationTicks) {
        return action.trigger(animName, durationTicks);
    }

    /** 当前招式动画名（无招式时为 null）；供 inspection / 测试。 */
    public String actionAnim() {
        return action.currentAnim();
    }

    /** 招式动画剩余 tick；供 inspection / 测试。 */
    public int actionTicks() {
        return action.remainingTicks();
    }

    @Override
    public boolean canHit() {
        return true;
    }

    @Override
    public void tick() {
        super.tick();
        // 一次性招式动画倒计时：归零后 action.currentAnim() 转 null，controller 回 idle。
        action.tick();
        updateHorizontalSpeed();
    }

    /**
     * 逐 tick 刷新水平速度信号。瞬时位移直接上冲，包间隙按 0.72 衰减维持——
     * 移动中速度稳定高于阈值（walk/run 不闪回 idle），停下后约几 tick 衰减归零（回 idle）。
     */
    private void updateHorizontalSpeed() {
        double x = getX();
        double z = getZ();
        if (hasLastTick) {
            double dx = x - lastTickX;
            double dz = z - lastTickZ;
            float inst = (float) Math.sqrt(dx * dx + dz * dz);
            horizontalSpeed = Math.max(inst, horizontalSpeed * SPEED_DECAY);
            if (inst > FACING_MIN_STEP) {
                // 与 server navigator 同一 yaw 约定（atan2(dz,dx)-90），保证客户端自算朝向与
                // 服务端一致，只是不依赖 marker 下发；平滑转身避免瞬移。
                float target = (float) (Math.toDegrees(Math.atan2(dz, dx)) - 90.0);
                movementYaw = FaunaYawMath.approachYaw(movementYaw, target, FACING_TURN_DEGREES_PER_TICK);
            }
        }
        lastTickX = x;
        lastTickZ = z;
        hasLastTick = true;
    }

    /** 当前平滑后的水平速度（blocks/tick）；供 controller 与测试读取。 */
    public float horizontalSpeed() {
        return horizontalSpeed;
    }

    /** 客户端按位移算出的移动朝向（MC yaw，度）；供渲染层面朝移动方向。 */
    public float movementYaw() {
        return movementYaw;
    }

    @Override
    public void registerControllers(AnimatableManager.ControllerRegistrar controllers) {
        // 按物种派生 idle 动画名：专属模型物种（animPath!=null）加载的是各自动画文件，其中没有
        // 通用 animation.fauna.idle → 若硬编码会解析不到 → 实体定格 T-Pose。见 FaunaVisualKind.idleAnimationName()。
        RawAnimation idle = RawAnimation.begin().thenLoop(visualKind.idleAnimationName());
        // walk/run 仅对声明了对应动画的物种生效（null=没有该动画，退回上一档），
        // 否则对没有 walk 的物种 setAnimation 一个缺失 key 会让 GeckoLib 解析失败 → T-Pose。
        String walkName = visualKind.walkAnimationName();
        String runName = visualKind.runAnimationName();
        RawAnimation walk = walkName == null ? null : RawAnimation.begin().thenLoop(walkName);
        RawAnimation run = runName == null ? null : RawAnimation.begin().thenLoop(runName);
        controllers.add(new AnimationController<>(this, "main", 5, state -> {
            // 优先级：一次性招式 > run > walk > idle。
            // 重建 RawAnimation 每帧都相等（同名循环/播放），GeckoLib 不会重启动画。
            String current = action.currentAnim();
            if (current != null) {
                state.getController().setAnimation(RawAnimation.begin().thenPlay(current));
                return PlayState.CONTINUE;
            }
            float speed = horizontalSpeed;
            if (run != null && speed >= RUN_SPEED_THRESHOLD) {
                state.getController().setAnimation(run);
            } else if (walk != null && speed >= WALK_SPEED_THRESHOLD) {
                state.getController().setAnimation(walk);
            } else {
                state.getController().setAnimation(idle);
            }
            return PlayState.CONTINUE;
        }));
    }

    @Override
    public AnimatableInstanceCache getAnimatableInstanceCache() {
        return cache;
    }

    @Override
    protected void initDataTracker() {
    }

    @Override
    protected void readCustomDataFromNbt(NbtCompound nbt) {
    }

    @Override
    protected void writeCustomDataToNbt(NbtCompound nbt) {
    }
}
