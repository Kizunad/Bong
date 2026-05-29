package com.bong.client.inventory;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.hit.EntityHitResult;

import java.util.Optional;

/**
 * Detects supply coffin entities under the crosshair and dispatches a C2S
 * {@code supply_coffin_open} request when the player interacts.
 *
 * <p>Supply coffins are Marker entities with no server-side hitbox, so the
 * vanilla {@code InteractEntityEvent} never fires. This handler replicates the
 * pattern used by {@link com.bong.client.npc.NpcEngagementIntentHandler}: the
 * client detects the entity via {@code crosshairTarget}, then sends a C2S
 * request that the server handles in {@code client_request_handler.rs}.</p>
 */
public final class SupplyCoffinInteractIntentHandler implements IntentHandler {
    private static final double MAX_INTERACT_DISTANCE_SQ = 5.0 * 5.0;

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        EntityHitResult hit = entityHit(client);
        if (hit == null) {
            return Optional.empty();
        }
        if (!(hit.getEntity() instanceof BongModeledEntity modeled)) {
            return Optional.empty();
        }
        BongEntityModelKind kind = modeled.modelKind();
        if (kind != BongEntityModelKind.COFFIN_COMMON
            && kind != BongEntityModelKind.COFFIN_RARE
            && kind != BongEntityModelKind.COFFIN_PRECIOUS) {
            return Optional.empty();
        }
        double distSq = client.player.squaredDistanceTo(hit.getEntity());
        if (distSq > MAX_INTERACT_DISTANCE_SQ) {
            return Optional.empty();
        }
        return Optional.of(InteractCandidate.of(
            InteractIntent.OpenContainer,
            ReservedInteractionIntents.OPEN_CONTAINER_PRIORITY,
            distSq,
            "supply_coffin:" + hit.getEntity().getId()
        ));
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        int candidateEntityId = candidateEntityId(candidate);
        if (candidateEntityId < 0) {
            return false;
        }
        EntityHitResult hit = entityHit(client);
        if (hit == null || hit.getEntity().getId() != candidateEntityId) {
            return false;
        }
        ClientRequestSender.sendSupplyCoffinOpen(candidateEntityId);
        return true;
    }

    private static int candidateEntityId(InteractCandidate candidate) {
        if (candidate == null || candidate.debugLabel() == null) {
            return -1;
        }
        String prefix = "supply_coffin:";
        if (!candidate.debugLabel().startsWith(prefix)) {
            return -1;
        }
        try {
            return Integer.parseInt(candidate.debugLabel().substring(prefix.length()));
        } catch (NumberFormatException exception) {
            return -1;
        }
    }

    private static EntityHitResult entityHit(MinecraftClient client) {
        if (client == null || client.player == null) {
            return null;
        }
        if (!(client.crosshairTarget instanceof EntityHitResult hit)) {
            return null;
        }
        return hit;
    }
}
