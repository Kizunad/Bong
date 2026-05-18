package com.bong.client.dandao;

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

/**
 * plan-dandao-path-v1 P4 -- Baolongwang (暴龙王) BOSS entity.
 *
 * <p>Client-side GeckoLib entity for the ultimate dandao mutation form.
 * Body dimensions: 4x4x6 blocks. 5 animations mapped from the Bedrock model.
 *
 * <p>Phase B-1: client-only debug entity for render verification.
 * Phase B-2: server Valence EntityKind sync will drive position/state.
 */
public final class BaolongwangEntity extends Entity implements GeoEntity {
    /** Idle breathing loop, 4.76s. */
    static final RawAnimation IDLE = RawAnimation.begin().thenLoop("idle");
    /** Walk cycle, 1.12s. */
    static final RawAnimation WALK = RawAnimation.begin().thenLoop("walk");
    /** Basic attack, 1.64s. */
    static final RawAnimation ATTACK = RawAnimation.begin().thenPlay("attack");
    /** Skill 1 (pill mist), 3.2s. */
    static final RawAnimation SKILL1 = RawAnimation.begin().thenPlay("skill1");
    /** Skill 2 (tail swipe), 1.64s. */
    static final RawAnimation SKILL2 = RawAnimation.begin().thenPlay("skill2");

    private final AnimatableInstanceCache cache = GeckoLibUtil.createInstanceCache(this);

    public BaolongwangEntity(EntityType<? extends BaolongwangEntity> type, World world) {
        super(type, world);
    }

    @Override
    public void registerControllers(AnimatableManager.ControllerRegistrar controllers) {
        controllers.add(new AnimationController<>(this, "main", 5, state -> {
            state.getController().setAnimation(IDLE);
            return PlayState.CONTINUE;
        }));
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
    }

    @Override
    protected void readCustomDataFromNbt(NbtCompound nbt) {
    }

    @Override
    protected void writeCustomDataToNbt(NbtCompound nbt) {
    }
}
