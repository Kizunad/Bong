package com.bong.client.alchemy;

import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import net.minecraft.util.math.BlockPos;

public final class AlchemyFurnaceInteractionRules {
    private AlchemyFurnaceInteractionRules() {
    }

    public static boolean shouldOpenAlchemyFurnace(BlockPos clickedPos, AlchemyFurnaceStore.Snapshot snapshot) {
        if (clickedPos == null || snapshot == null || snapshot.pos() == null) return false;
        return clickedPos.equals(snapshot.pos());
    }
}
