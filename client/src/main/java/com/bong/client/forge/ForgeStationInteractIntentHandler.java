package com.bong.client.forge;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.hit.EntityHitResult;
import net.minecraft.util.hit.HitResult;
import net.minecraft.util.math.BlockPos;

import java.util.Optional;

/** 炼器砧走统一交互键；方块与模型命中归一到服务端工位坐标。 */
public final class ForgeStationInteractIntentHandler implements IntentHandler {
    private static final String LABEL_PREFIX = "forge_station:";

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        BlockPos pos = target(client);
        if (pos == null) return Optional.empty();
        return Optional.of(InteractCandidate.of(
            InteractIntent.OpenContainer,
            ReservedInteractionIntents.OPEN_CONTAINER_PRIORITY,
            client.player.squaredDistanceTo(pos.getX(), pos.getY(), pos.getZ()),
            LABEL_PREFIX + pos.asLong()
        ));
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        BlockPos pos = target(client);
        if (pos == null || candidate == null || candidate.intent() != InteractIntent.OpenContainer
            || !candidate.debugLabel().equals(LABEL_PREFIX + pos.asLong())) return false;
        ClientRequestSender.sendForgeStationOpen(pos);
        return true;
    }

    private static BlockPos target(MinecraftClient client) {
        if (client == null || client.player == null || client.world == null || client.currentScreen != null) {
            return null;
        }
        BlockPos pos;
        if (client.crosshairTarget instanceof EntityHitResult hit
            && hit.getEntity() instanceof BongModeledEntity modeled
            && !modeled.isRemoved() && modeled.modelKind() == BongEntityModelKind.FORGE_STATION) {
            pos = modeled.getBlockPos();
        } else if (client.crosshairTarget instanceof BlockHitResult hit && hit.getType() == HitResult.Type.BLOCK) {
            pos = hit.getBlockPos();
        } else {
            return null;
        }
        // 方块命中也必须有真实工位模型，普通铁砧不能冒充锻造工位。
        return ForgeScreenBootstrap.available(client, pos) ? pos : null;
    }
}
