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
    private final FaunaPlayback playback;
    private int renderedRevision = -1;
    private double previousHp = Double.NaN;
    private Boolean previewDisguise;

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
        playback = new FaunaPlayback(visualKind);
    }

    public FaunaVisualKind visualKind() {
        return visualKind;
    }

    /**
     * 触发真实存在的动作（由网络层在主线程调用），到期后回到当前形态的待机/移动。
     *
     * @param animName      GeckoLib 动画名（如 {@code animation.bong.heiwushi.dark_barrage}）
     * @param durationTicks 动画占用时长（tick）
     * @return 动作存在且时长有效时返回 true；未知动作保持当前播放状态。
     */
    public boolean triggerAction(String animName, int durationTicks) {
        return playback.trigger(animName, durationTicks);
    }

    /** 当前招式动画名（无招式时为 null）；供 inspection / 测试。 */
    public String actionAnim() {
        return playback.actionName();
    }

    /** 招式动画剩余 tick；供 inspection / 测试。 */
    public int actionTicks() {
        return playback.remainingTicks();
    }

    @Override
    public boolean canHit() {
        return true;
    }

    @Override
    public void tick() {
        super.tick();
        // 动作结束后按当前形态与速度继续播放。
        playback.tick();
        if (previewDisguise == null) playback.syncSpiderDisguise(getId());
        else playback.spiderDisguise(previewDisguise, true);
        var metadata = com.bong.client.npc.NpcMetadataStore.get(getId());
        if (metadata != null) {
            double hp = metadata.hpRatio();
            if (!Double.isNaN(previousHp) && hp < previousHp) {
                if (hp <= 0) {
                    var death = playback.profile().find("death");
                    if (death == null) death = playback.profile().find("die");
                    if (death != null) playback.trigger(death.name(), Integer.MAX_VALUE);
                } else if (playback.actionName() == null) {
                    playback.play("hurt");
                }
            }
            previousHp = hp;
        }
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
        if (!hasLastTick) movementYaw = getYaw();
        hasLastTick = true;
    }

    /** 当前平滑后的水平速度（blocks/tick）；供 controller 与测试读取。 */
    public float horizontalSpeed() {
        return horizontalSpeed;
    }

    /** 客户端按位移算出的移动朝向（MC yaw，度）；供渲染层面朝移动方向。 */
    public float movementYaw() {
        return hasLastTick ? movementYaw : getYaw();
    }

    public FaunaPlayback playback() {
        return playback;
    }

    public void togglePreviewDisguise() {
        if (getId() < 0) previewDisguise = !Boolean.TRUE.equals(previewDisguise);
    }

    @Override
    public void registerControllers(AnimatableManager.ControllerRegistrar controllers) {
        controllers.add(new AnimationController<>(this, "main", 5, state -> {
            var controller = state.getController();
            if (renderedRevision != playback.revision()) {
                // 连续两次同名攻击也必须重播；形态切换不能沿用旧骨架的动画队列。
                controller.forceAnimationReset();
                renderedRevision = playback.revision();
            }
            var clip = playback.current(horizontalSpeed);
            boolean burst = FaunaAnimations.shortName(clip.name()).equals("ambush_burst");
            controller.transitionLength(burst ? 0 : 5);
            if (playback.blockDisguise()) return PlayState.STOP;
            controller.setAnimation(clip.loop()
                ? RawAnimation.begin().thenLoop(clip.name())
                : RawAnimation.begin().thenPlayAndHold(clip.name()));
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
