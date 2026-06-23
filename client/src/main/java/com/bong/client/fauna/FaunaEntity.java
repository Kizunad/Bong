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

    public FaunaEntity(EntityType<? extends FaunaEntity> type, World world, FaunaVisualKind visualKind) {
        super(type, world);
        this.visualKind = Objects.requireNonNull(visualKind, "visualKind");
    }

    public FaunaVisualKind visualKind() {
        return visualKind;
    }

    @Override
    public boolean canHit() {
        return true;
    }

    @Override
    public void registerControllers(AnimatableManager.ControllerRegistrar controllers) {
        // 按物种派生 idle 动画名：专属模型物种（animPath!=null）加载的是各自动画文件，其中没有
        // 通用 animation.fauna.idle → 若硬编码会解析不到 → 实体定格 T-Pose。见 FaunaVisualKind.idleAnimationName()。
        RawAnimation idle = RawAnimation.begin().thenLoop(visualKind.idleAnimationName());
        controllers.add(new AnimationController<>(this, "main", 5, state -> {
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
