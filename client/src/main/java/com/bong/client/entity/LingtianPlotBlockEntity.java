package com.bong.client.entity;

import net.minecraft.block.BlockState;
import net.minecraft.block.entity.BlockEntity;
import net.minecraft.util.math.BlockPos;

public final class LingtianPlotBlockEntity extends BlockEntity {
    private LingtianPlotBlock.VisualState visualState = LingtianPlotBlock.VisualState.WILD;

    public LingtianPlotBlockEntity(BlockPos pos, BlockState state) {
        super(LingtianPlotBlock.entityType(), pos, state);
    }

    public LingtianPlotBlock.VisualState visualState() {
        return visualState;
    }

    public void setVisualState(LingtianPlotBlock.VisualState visualState) {
        this.visualState = LingtianPlotBlock.normalize(visualState);
        markDirty();
    }

    public int textureState() {
        return visualState.textureState();
    }
}
