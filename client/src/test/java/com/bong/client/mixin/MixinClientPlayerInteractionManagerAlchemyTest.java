package com.bong.client.mixin;

import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MixinClientPlayerInteractionManagerAlchemyTest {

    @Test
    void onlyKnownFurnacePositionOpensAlchemyUi() {
        BlockPos known = new BlockPos(-12, 64, 38);
        AlchemyFurnaceStore.Snapshot snapshot = new AlchemyFurnaceStore.Snapshot(
            known, 1, 92f, 100f, "self", false
        );

        assertTrue(MixinClientPlayerInteractionManagerAlchemy.shouldOpenAlchemyFurnace(known, snapshot));
        assertFalse(MixinClientPlayerInteractionManagerAlchemy.shouldOpenAlchemyFurnace(new BlockPos(-12, 64, 39), snapshot));
        assertFalse(MixinClientPlayerInteractionManagerAlchemy.shouldOpenAlchemyFurnace(known, AlchemyFurnaceStore.Snapshot.empty()));
    }
}
