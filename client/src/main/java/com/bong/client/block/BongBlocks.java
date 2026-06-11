package com.bong.client.block;

import net.fabricmc.fabric.api.object.builder.v1.block.FabricBlockSettings;
import net.minecraft.block.AbstractBlock;
import net.minecraft.block.Block;
import net.minecraft.block.Blocks;
import net.minecraft.block.PillarBlock;
import net.minecraft.registry.Registries;
import net.minecraft.registry.Registry;
import net.minecraft.state.StateManager;
import net.minecraft.state.property.BooleanProperty;
import net.minecraft.state.property.Properties;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Direction;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;
import java.util.Map;

public final class BongBlocks {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-blocks");

    public static final Identifier ZHENFA_NODE_ID = id("zhenfa_node");
    public static final Identifier ZHENFA_LINE_ID = id("zhenfa_line");
    public static final Identifier ZHENFA_EYE_ID = id("zhenfa_eye");
    public static final Identifier SIMPLE_BED_ID = id("simple_bed");
    public static final Identifier MEDITATION_MAT_ID = id("meditation_mat");
    public static final Identifier MOISTURE_BASE_ID = id("moisture_base");
    public static final Identifier SPIRIT_STONE_RACK_ID = id("spirit_stone_rack");
    public static final BooleanProperty CHARGED = BooleanProperty.of("charged");

    public static final Block ZHENFA_NODE = new Block(settings(3));
    public static final PillarBlock ZHENFA_LINE = new PillarBlock(settings(1));
    public static final Block ZHENFA_EYE = new ZhenfaEyeBlock(
        FabricBlockSettings.copyOf(Blocks.GLASS)
            .luminance(state -> state.get(CHARGED) ? 8 : 5)
            .noCollision()
            .breakInstantly()
            .nonOpaque()
    );
    public static final Block SIMPLE_BED = new Block(settings(0));
    public static final Block MEDITATION_MAT = new Block(settings(0));
    public static final Block MOISTURE_BASE = new Block(settings(0));
    public static final Block SPIRIT_STONE_RACK = new Block(settings(2));

    private static boolean registered;

    private BongBlocks() {}

    public static synchronized void register() {
        if (registered) {
            return;
        }

        registerBlock(ZHENFA_NODE_ID, ZHENFA_NODE);
        registerBlock(ZHENFA_LINE_ID, ZHENFA_LINE);
        registerBlock(ZHENFA_EYE_ID, ZHENFA_EYE);
        registerBlock(SIMPLE_BED_ID, SIMPLE_BED);
        registerBlock(MEDITATION_MAT_ID, MEDITATION_MAT);
        registerBlock(MOISTURE_BASE_ID, MOISTURE_BASE);
        registerBlock(SPIRIT_STONE_RACK_ID, SPIRIT_STONE_RACK);
        registered = true;
        verifyRawIds();

        LOGGER.info(
            "Registered Bong custom blocks: {}, {}, {}, {}, {}, {}, {}",
            ZHENFA_NODE_ID,
            ZHENFA_LINE_ID,
            ZHENFA_EYE_ID,
            SIMPLE_BED_ID,
            MEDITATION_MAT_ID,
            MOISTURE_BASE_ID,
            SPIRIT_STONE_RACK_ID
        );
    }

    public static List<Identifier> orderedIdsForTests() {
        return List.of(
            ZHENFA_NODE_ID,
            ZHENFA_LINE_ID,
            ZHENFA_EYE_ID,
            SIMPLE_BED_ID,
            MEDITATION_MAT_ID,
            MOISTURE_BASE_ID,
            SPIRIT_STONE_RACK_ID
        );
    }

    public static Map<String, Integer> expectedBlockRawIdsForTests() {
        return Map.of(
            "zhenfa_node", BongBlockIds.ZHENFA_NODE_BLOCK_ID,
            "zhenfa_line", BongBlockIds.ZHENFA_LINE_BLOCK_ID,
            "zhenfa_eye", BongBlockIds.ZHENFA_EYE_BLOCK_ID,
            "simple_bed", BongBlockIds.SIMPLE_BED_BLOCK_ID,
            "meditation_mat", BongBlockIds.MEDITATION_MAT_BLOCK_ID,
            "moisture_base", BongBlockIds.MOISTURE_BASE_BLOCK_ID,
            "spirit_stone_rack", BongBlockIds.SPIRIT_STONE_RACK_BLOCK_ID
        );
    }

    public static Map<String, Integer> expectedStateRawIdsForTests() {
        return Map.ofEntries(
            Map.entry("zhenfa_node", BongBlockIds.ZHENFA_NODE_STATE_ID),
            Map.entry("zhenfa_line_axis_x", BongBlockIds.ZHENFA_LINE_STATE_ID),
            Map.entry("zhenfa_line_axis_y", BongBlockIds.ZHENFA_LINE_STATE_ID + 1),
            Map.entry("zhenfa_line_axis_z", BongBlockIds.ZHENFA_LINE_STATE_ID + 2),
            Map.entry("zhenfa_eye_charged_true", BongBlockIds.ZHENFA_EYE_STATE_ID),
            Map.entry("zhenfa_eye_charged_false", BongBlockIds.ZHENFA_EYE_STATE_ID + 1),
            Map.entry("simple_bed", BongBlockIds.SIMPLE_BED_STATE_ID),
            Map.entry("meditation_mat", BongBlockIds.MEDITATION_MAT_STATE_ID),
            Map.entry("moisture_base", BongBlockIds.MOISTURE_BASE_STATE_ID),
            Map.entry("spirit_stone_rack", BongBlockIds.SPIRIT_STONE_RACK_STATE_ID)
        );
    }

    private static AbstractBlock.Settings settings(int luminance) {
        return FabricBlockSettings.copyOf(Blocks.GLASS)
            .luminance(luminance)
            .noCollision()
            .breakInstantly()
            .nonOpaque();
    }

    private static Block registerBlock(Identifier id, Block block) {
        return Registry.register(Registries.BLOCK, id, block);
    }

    private static Identifier id(String path) {
        return new Identifier("bong", path);
    }

    private static void verifyRawIds() {
        verifyBlockRawId("zhenfa_node", ZHENFA_NODE, BongBlockIds.ZHENFA_NODE_BLOCK_ID);
        verifyBlockRawId("zhenfa_line", ZHENFA_LINE, BongBlockIds.ZHENFA_LINE_BLOCK_ID);
        verifyBlockRawId("zhenfa_eye", ZHENFA_EYE, BongBlockIds.ZHENFA_EYE_BLOCK_ID);
        verifyBlockRawId("simple_bed", SIMPLE_BED, BongBlockIds.SIMPLE_BED_BLOCK_ID);
        verifyBlockRawId("meditation_mat", MEDITATION_MAT, BongBlockIds.MEDITATION_MAT_BLOCK_ID);
        verifyBlockRawId("moisture_base", MOISTURE_BASE, BongBlockIds.MOISTURE_BASE_BLOCK_ID);
        verifyBlockRawId("spirit_stone_rack", SPIRIT_STONE_RACK, BongBlockIds.SPIRIT_STONE_RACK_BLOCK_ID);

        verifyStateRawId("zhenfa_node", ZHENFA_NODE.getDefaultState(), BongBlockIds.ZHENFA_NODE_STATE_ID);
        verifyStateRawId(
            "zhenfa_line axis=x",
            ZHENFA_LINE.getDefaultState().with(Properties.AXIS, Direction.Axis.X),
            BongBlockIds.ZHENFA_LINE_STATE_ID
        );
        verifyStateRawId(
            "zhenfa_line axis=y",
            ZHENFA_LINE.getDefaultState().with(Properties.AXIS, Direction.Axis.Y),
            BongBlockIds.ZHENFA_LINE_STATE_ID + 1
        );
        verifyStateRawId(
            "zhenfa_line axis=z",
            ZHENFA_LINE.getDefaultState().with(Properties.AXIS, Direction.Axis.Z),
            BongBlockIds.ZHENFA_LINE_STATE_ID + 2
        );
        verifyStateRawId(
            "zhenfa_eye charged=true",
            ZHENFA_EYE.getDefaultState().with(CHARGED, true),
            BongBlockIds.ZHENFA_EYE_STATE_ID
        );
        verifyStateRawId(
            "zhenfa_eye charged=false",
            ZHENFA_EYE.getDefaultState().with(CHARGED, false),
            BongBlockIds.ZHENFA_EYE_STATE_ID + 1
        );
        verifyStateRawId("simple_bed", SIMPLE_BED.getDefaultState(), BongBlockIds.SIMPLE_BED_STATE_ID);
        verifyStateRawId(
            "meditation_mat",
            MEDITATION_MAT.getDefaultState(),
            BongBlockIds.MEDITATION_MAT_STATE_ID
        );
        verifyStateRawId("moisture_base", MOISTURE_BASE.getDefaultState(), BongBlockIds.MOISTURE_BASE_STATE_ID);
        verifyStateRawId(
            "spirit_stone_rack",
            SPIRIT_STONE_RACK.getDefaultState(),
            BongBlockIds.SPIRIT_STONE_RACK_STATE_ID
        );
    }

    private static void verifyBlockRawId(String name, Block block, int expectedRawId) {
        int rawId = Registries.BLOCK.getRawId(block);
        if (rawId != expectedRawId) {
            throw new IllegalStateException(
                "Bong block raw id mismatch for " + name + ": expected " + expectedRawId + ", got " + rawId
            );
        }
    }

    private static void verifyStateRawId(String name, net.minecraft.block.BlockState state, int expectedRawId) {
        int rawId = Block.STATE_IDS.getRawId(state);
        if (rawId != expectedRawId) {
            throw new IllegalStateException(
                "Bong block state raw id mismatch for " + name + ": expected " + expectedRawId + ", got " + rawId
            );
        }
    }

    private static final class ZhenfaEyeBlock extends Block {
        private ZhenfaEyeBlock(AbstractBlock.Settings settings) {
            super(settings);
            setDefaultState(getStateManager().getDefaultState().with(CHARGED, false));
        }

        @Override
        protected void appendProperties(StateManager.Builder<Block, net.minecraft.block.BlockState> builder) {
            builder.add(CHARGED);
        }
    }
}
