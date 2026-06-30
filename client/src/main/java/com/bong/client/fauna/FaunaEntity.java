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
    private final AnimatableInstanceCache cache = GeckoLibUtil.createInstanceCache(this);
    private final FaunaVisualKind visualKind;

    /**
     * 一次性招式动画状态机（黑武士 boss 出招）。由服务端 {@code play_entity_anim} 经
     * {@code VfxEventRouter} → {@code FaunaActionBridge} → {@link #triggerAction} 驱动。
     */
    private final FaunaActionAnimation action = new FaunaActionAnimation();

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
     */
    public void triggerAction(String animName, int durationTicks) {
        action.trigger(animName, durationTicks);
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
    }

    @Override
    public void registerControllers(AnimatableManager.ControllerRegistrar controllers) {
        // 按物种派生 idle 动画名：专属模型物种（animPath!=null）加载的是各自动画文件，其中没有
        // 通用 animation.fauna.idle → 若硬编码会解析不到 → 实体定格 T-Pose。见 FaunaVisualKind.idleAnimationName()。
        RawAnimation idle = RawAnimation.begin().thenLoop(visualKind.idleAnimationName());
        controllers.add(new AnimationController<>(this, "main", 5, state -> {
            // 一次性招式动画优先于 idle（参照 BaolongwangEntity 的优先级写法）。
            // 重建 RawAnimation 每帧都相等（同名 thenPlay），GeckoLib 不会重启动画。
            String current = action.currentAnim();
            if (current != null) {
                state.getController().setAnimation(RawAnimation.begin().thenPlay(current));
                return PlayState.CONTINUE;
            }
            state.getController().setAnimation(idle);
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
