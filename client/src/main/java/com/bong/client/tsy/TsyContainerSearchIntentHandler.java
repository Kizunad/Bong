package com.bong.client.tsy;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.Entity;
import net.minecraft.util.hit.EntityHitResult;

import java.util.Optional;

public final class TsyContainerSearchIntentHandler implements IntentHandler {
    public static final double MAX_INTERACT_DISTANCE = 3.0;
    private static final String DEBUG_PREFIX = "tsy_container:";

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        EntityHitResult hit = entityHit(client);
        if (hit == null) {
            return Optional.empty();
        }
        Integer visualEntityId = visualEntityId(hit.getEntity());
        if (visualEntityId == null) {
            return Optional.empty();
        }
        return candidateForVisualHit(
            visualEntityId,
            client.player.getX(),
            client.player.getY(),
            client.player.getZ()
        );
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        EntityHitResult hit = entityHit(client);
        if (hit == null) {
            return false;
        }
        Integer visualEntityId = visualEntityId(hit.getEntity());
        if (visualEntityId == null) {
            return false;
        }
        Long containerEntityId = dispatchEntityIdForVisualHit(
            candidate,
            visualEntityId,
            client.player.getX(),
            client.player.getY(),
            client.player.getZ()
        );
        if (containerEntityId == null) {
            return false;
        }
        ClientRequestSender.sendStartSearch(containerEntityId);
        return true;
    }

    static Optional<InteractCandidate> candidateForVisualHit(
        int visualEntityId,
        double playerX,
        double playerY,
        double playerZ
    ) {
        TsyContainerView container = containerForVisualHit(visualEntityId, playerX, playerY, playerZ);
        if (container == null) {
            return Optional.empty();
        }
        return Optional.of(InteractCandidate.of(
            InteractIntent.SearchContainer,
            ReservedInteractionIntents.SEARCH_CONTAINER_PRIORITY,
            container.distanceSq(playerX, playerY, playerZ),
            DEBUG_PREFIX + container.entityId()
        ));
    }

    static Long dispatchEntityIdForVisualHit(
        InteractCandidate candidate,
        int visualEntityId,
        double playerX,
        double playerY,
        double playerZ
    ) {
        Long candidateEntityId = candidateEntityId(candidate);
        if (candidateEntityId == null) {
            return null;
        }
        TsyContainerView container = containerForVisualHit(visualEntityId, playerX, playerY, playerZ);
        if (container == null || container.entityId() != candidateEntityId) {
            return null;
        }
        return candidateEntityId;
    }

    static Long candidateEntityId(InteractCandidate candidate) {
        if (candidate == null || candidate.debugLabel() == null) {
            return null;
        }
        if (!candidate.debugLabel().startsWith(DEBUG_PREFIX)) {
            return null;
        }
        try {
            return Long.parseLong(candidate.debugLabel().substring(DEBUG_PREFIX.length()));
        } catch (NumberFormatException exception) {
            return null;
        }
    }

    static boolean isTsyContainerVisualKind(BongEntityModelKind kind) {
        if (kind == null) {
            return false;
        }
        return switch (kind) {
            case DRY_CORPSE, BONE_SKELETON, STORAGE_POUCH, STONE_CASKET -> true;
            default -> false;
        };
    }

    private static TsyContainerView containerForVisualHit(
        int visualEntityId,
        double playerX,
        double playerY,
        double playerZ
    ) {
        TsyContainerView container = TsyContainerStateStore.getByVisualEntityId(visualEntityId);
        if (container == null || !container.interactable()) {
            return null;
        }
        if (container.distanceSq(playerX, playerY, playerZ) > MAX_INTERACT_DISTANCE * MAX_INTERACT_DISTANCE) {
            return null;
        }
        return container;
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

    private static Integer visualEntityId(Entity entity) {
        if (!(entity instanceof BongModeledEntity modeled)) {
            return null;
        }
        if (!isTsyContainerVisualKind(modeled.modelKind())) {
            return null;
        }
        return entity.getId();
    }
}
