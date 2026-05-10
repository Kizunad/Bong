package com.bong.client.entity;

import net.minecraft.entity.Entity;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.data.DataTracker;
import net.minecraft.entity.data.TrackedData;
import net.minecraft.entity.data.TrackedDataHandlerRegistry;
import net.minecraft.nbt.NbtCompound;
import net.minecraft.world.World;
import software.bernie.geckolib.animatable.GeoEntity;
import software.bernie.geckolib.core.animatable.instance.AnimatableInstanceCache;
import software.bernie.geckolib.core.animation.AnimatableManager;
import software.bernie.geckolib.core.animation.AnimationController;
import software.bernie.geckolib.core.animation.RawAnimation;
import software.bernie.geckolib.core.object.PlayState;
import software.bernie.geckolib.util.GeckoLibUtil;

public final class BongModeledEntity extends Entity implements GeoEntity {
    private static final TrackedData<Integer> VISUAL_STATE =
        DataTracker.registerData(BongModeledEntity.class, TrackedDataHandlerRegistry.INTEGER);

    private final BongEntityModelKind modelKind;
    private final AnimatableInstanceCache cache = GeckoLibUtil.createInstanceCache(this);

    public BongModeledEntity(
        EntityType<? extends BongModeledEntity> type,
        World world,
        BongEntityModelKind modelKind
    ) {
        super(type, world);
        this.modelKind = modelKind;
        this.noClip = true;
        this.setNoGravity(true);
    }

    public static EntityType.EntityFactory<BongModeledEntity> factory(BongEntityModelKind modelKind) {
        return (type, world) -> new BongModeledEntity(type, world, modelKind);
    }

    public BongEntityModelKind modelKind() {
        return modelKind;
    }

    public int visualState() {
        return dataTracker.get(VISUAL_STATE);
    }

    public void setVisualState(int visualState) {
        int stateCount = modelKind.stateCount();
        if (stateCount <= 0) {
            dataTracker.set(VISUAL_STATE, 0);
            return;
        }
        dataTracker.set(VISUAL_STATE, Math.floorMod(visualState, stateCount));
    }

    @Override
    public void registerControllers(AnimatableManager.ControllerRegistrar controllers) {
        RawAnimation idle = RawAnimation.begin().thenLoop(modelKind.idleAnimationName());
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
        dataTracker.startTracking(VISUAL_STATE, 0);
    }

    @Override
    protected void readCustomDataFromNbt(NbtCompound nbt) {
        setVisualState(nbt.getInt("VisualState"));
    }

    @Override
    protected void writeCustomDataToNbt(NbtCompound nbt) {
        nbt.putInt("VisualState", visualState());
    }
}
