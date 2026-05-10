package com.bong.client.entity;

import net.minecraft.util.math.BlockPos;

public record LingtianPlotBlockEntity(BlockPos pos, LingtianPlotBlock.VisualState state) {
    public LingtianPlotBlockEntity {
        if (state == null) {
            state = LingtianPlotBlock.VisualState.WILD;
        }
    }

    public int textureState() {
        return state.textureState();
    }
}
