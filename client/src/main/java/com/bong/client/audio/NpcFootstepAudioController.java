package com.bong.client.audio;

import com.bong.client.network.AudioEventPayload;
import com.bong.client.npc.NpcMetadata;
import com.bong.client.npc.NpcMetadataStore;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.block.BlockState;
import net.minecraft.block.Blocks;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.Entity;
import net.minecraft.fluid.Fluids;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;

public final class NpcFootstepAudioController {
    private static final int STEP_INTERVAL_TICKS = 8;
    private static final double MIN_MOVE_SQ = 0.04;
    private static final Map<Integer, StepState> STATES = new HashMap<>();

    private NpcFootstepAudioController() {
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(NpcFootstepAudioController::tick);
    }

    static AudioRecipe recipeForMaterial(String material) {
        String recipeId = switch (material) {
            case "ash" -> "npc_footstep_ash";
            case "water" -> "npc_footstep_water";
            default -> "npc_footstep_default";
        };
        return new AudioRecipe(
            recipeId,
            switch (recipeId) {
                case "npc_footstep_ash" -> List.of(
                    new AudioLayer(new Identifier("minecraft", "entity.player.swim"), 0.08f, 1.05f, 0),
                    new AudioLayer(new Identifier("minecraft", "block.sand.step"), 0.05f, 0.7f, 0)
                );
                case "npc_footstep_water" -> List.of(
                    new AudioLayer(new Identifier("minecraft", "entity.player.swim"), 0.08f, 1.2f, 0),
                    new AudioLayer(new Identifier("minecraft", "entity.player.splash"), 0.03f, 2.0f, 0)
                );
                default -> List.of(new AudioLayer(new Identifier("minecraft", "entity.player.swim"), 0.1f, 1.5f, 0));
            },
            Optional.empty(),
            28,
            AudioAttenuation.MELEE,
            AudioCategory.PLAYERS,
            AudioBus.ENVIRONMENT
        );
    }

    private static void tick(MinecraftClient client) {
        if (client == null || client.world == null) {
            STATES.clear();
            return;
        }
        long tick = client.world.getTime();
        for (NpcMetadata metadata : NpcMetadataStore.snapshot()) {
            Entity entity = client.world.getEntityById(metadata.entityId());
            if (entity == null) {
                STATES.remove(metadata.entityId());
                continue;
            }
            StepState previous = STATES.get(metadata.entityId());
            double x = entity.getX();
            double y = entity.getY();
            double z = entity.getZ();
            if (previous != null
                && tick < previous.nextTick
                || previous != null && horizontalDistanceSq(previous, x, z) < MIN_MOVE_SQ) {
                STATES.put(metadata.entityId(), new StepState(x, y, z, previous == null ? tick + STEP_INTERVAL_TICKS : previous.nextTick));
                continue;
            }
            BlockPos blockPos = entity.getBlockPos().down();
            String material = materialFor(client.world.getBlockState(blockPos));
            AudioRecipe recipe = recipeForMaterial(material);
            SoundRecipePlayer.instance().play(new AudioEventPayload.PlaySoundRecipe(
                recipe.id(),
                90_000L + metadata.entityId(),
                Optional.of(new AudioPosition(blockPos.getX(), blockPos.getY(), blockPos.getZ())),
                Optional.empty(),
                1.0f,
                0.0f,
                recipe
            ));
            STATES.put(metadata.entityId(), new StepState(x, y, z, tick + STEP_INTERVAL_TICKS));
        }
    }

    private static String materialFor(BlockState state) {
        if (state == null) {
            return "default";
        }
        if (state.getFluidState().isOf(Fluids.WATER)) {
            return "water";
        }
        if (state.isOf(Blocks.SAND) || state.isOf(Blocks.RED_SAND) || state.isOf(Blocks.SOUL_SAND)) {
            return "ash";
        }
        return "default";
    }

    private static double horizontalDistanceSq(StepState previous, double x, double z) {
        double dx = previous.x - x;
        double dz = previous.z - z;
        return dx * dx + dz * dz;
    }

    private record StepState(double x, double y, double z, long nextTick) {
    }
}
