package com.bong.client.entity;

import com.bong.client.alchemy.AlchemyResultEffect;
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
    private String transitionAnimation;
    private int transitionTicks;
    private int lastObservedVisualState;
    private AnimationController<BongModeledEntity> mainController;
    private boolean playingAlchemyResult;
    private float alchemyHeat;

    public float alchemyHeat() { return alchemyHeat; }

    public void setAlchemyHeat(float heat) { alchemyHeat = Math.max(0, Math.min(1, heat)); }

    /** 清除炼丹世界表现，让炉体回到无炉次时的常态。 */
    public void resetAlchemyEffects() {
        setAlchemyHeat(0);
        transitionAnimation = null;
        transitionTicks = 0;
        playingAlchemyResult = false;
        if (mainController != null) mainController.forceAnimationReset();
    }

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
        dataTracker.set(VISUAL_STATE, modelKind.normalizeVisualState(visualState));
        observeVisualStateChange();
    }

    @Override
    public void tick() {
        super.tick();
        if (transitionTicks > 0 && --transitionTicks == 0) {
            transitionAnimation = null;
            playingAlchemyResult = false;
        }
    }

    @Override
    public void onTrackedDataSet(TrackedData<?> data) {
        super.onTrackedDataSet(data);
        if (VISUAL_STATE.equals(data)) {
            observeVisualStateChange();
        }
    }

    @Override
    public void registerControllers(AnimatableManager.ControllerRegistrar controllers) {
        mainController = new AnimationController<>(this, "main", 2, state -> {
            if (transitionAnimation != null) {
                state.getController().setAnimation(RawAnimation.begin().thenPlay(transitionAnimation));
            } else {
                state.getController().setAnimation(
                    RawAnimation.begin().thenLoop(modelKind.animationNameForState(
                        modelKind == BongEntityModelKind.ALCHEMY_FURNACE && alchemyHeat <= .01f ? 0 : visualState()))
                );
            }
            return PlayState.CONTINUE;
        });
        controllers.add(mainController);
    }

    /** 工位预览及收到服务端确认的世界实体共用；不改变服务端状态。 */
    public void playAlchemyFeed() {
        if (modelKind != BongEntityModelKind.ALCHEMY_FURNACE) return;
        if (playingAlchemyResult) return;
        transitionAnimation = "animation.bong.alchemy_furnace.feed";
        transitionTicks = 16;
        if (mainController != null) mainController.forceAnimationReset();
    }

    public void playAlchemyResult(AlchemyResultEffect result) {
        if (modelKind != BongEntityModelKind.ALCHEMY_FURNACE || result == null) return;
        transitionAnimation = result.animation();
        transitionTicks = result.ticks();
        playingAlchemyResult = true;
        if (mainController != null) mainController.forceAnimationReset();
    }

    private void observeVisualStateChange() {
        int nextState = modelKind.normalizeVisualState(visualState());
        if (nextState == lastObservedVisualState) {
            return;
        }
        if (playingAlchemyResult) {
            // 世界实体的空炉 metadata 可能晚于炸炉 VFX，不能用合盖动画覆盖炸炉。
            lastObservedVisualState = nextState;
            return;
        }
        transitionAnimation = modelKind.transitionAnimationName(lastObservedVisualState, nextState);
        transitionTicks = transitionAnimation == null ? 0 : modelKind.transitionAnimationTicks();
        lastObservedVisualState = nextState;
    }

    @Override
    public AnimatableInstanceCache getAnimatableInstanceCache() {
        return cache;
    }

    @Override
    public boolean canHit() {
        return true;
    }

    @Override
    protected void initDataTracker() {
        dataTracker.startTracking(VISUAL_STATE, 0);
    }

    @Override
    protected void readCustomDataFromNbt(NbtCompound nbt) {
        // EntityType is disableSaving(); visual state is server metadata, not local NBT.
    }

    @Override
    protected void writeCustomDataToNbt(NbtCompound nbt) {
        // EntityType is disableSaving(); visual state is server metadata, not local NBT.
    }
}
