package com.bong.client.entity;

import net.fabricmc.fabric.api.object.builder.v1.block.FabricBlockSettings;
import net.fabricmc.fabric.api.object.builder.v1.block.entity.FabricBlockEntityTypeBuilder;
import net.minecraft.block.BlockRenderType;
import net.minecraft.block.BlockState;
import net.minecraft.block.BlockWithEntity;
import net.minecraft.block.Blocks;
import net.minecraft.block.entity.BlockEntity;
import net.minecraft.block.entity.BlockEntityType;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;

public final class LingtianPlotBlock extends BlockWithEntity {
    public static final Identifier ID = new Identifier("bong", "lingtian_plot");
    private static BlockEntityType<LingtianPlotBlockEntity> entityType;
    private static boolean registered;

    private LingtianPlotBlock() {
        super(FabricBlockSettings.copyOf(Blocks.FARMLAND).nonOpaque());
    }

    public static void register() {
        if (registered) {
            return;
        }
        Registry.register(Registries.BLOCK, ID, Holder.INSTANCE);
        entityType = Registry.register(
            Registries.BLOCK_ENTITY_TYPE,
            ID,
            FabricBlockEntityTypeBuilder.create(LingtianPlotBlockEntity::new, Holder.INSTANCE).build(null)
        );
        registered = true;
    }

    public static LingtianPlotBlock instance() {
        return Holder.INSTANCE;
    }

    public static BlockEntityType<LingtianPlotBlockEntity> entityType() {
        if (entityType == null) {
            throw new IllegalStateException("LingtianPlotBlock.register() must run before creating block entities");
        }
        return entityType;
    }

    @Override
    public BlockEntity createBlockEntity(BlockPos pos, BlockState state) {
        return new LingtianPlotBlockEntity(pos, state);
    }

    @Override
    public BlockRenderType getRenderType(BlockState state) {
        return BlockRenderType.MODEL;
    }

    public enum VisualState {
        WILD(0),
        TILLED(1),
        PLANTED(2),
        MATURE(3);

        private final int textureState;

        VisualState(int textureState) {
            this.textureState = textureState;
        }

        public int textureState() {
            return textureState;
        }
    }

    public static BongEntityModelKind modelKind() {
        return BongEntityModelKind.LINGTIAN_PLOT;
    }

    public static VisualState normalize(VisualState state) {
        return state == null ? VisualState.WILD : state;
    }

    private static final class Holder {
        private static final LingtianPlotBlock INSTANCE = new LingtianPlotBlock();
    }
}
