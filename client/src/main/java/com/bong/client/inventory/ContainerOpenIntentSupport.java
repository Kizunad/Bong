package com.bong.client.inventory;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.hit.EntityHitResult;

import java.util.Optional;
import java.util.function.Predicate;

final class ContainerOpenIntentSupport {
    /**
     * 世界容器交互半径的平方（方块数），必须与 server 侧
     * {@code OPEN_RANGE_BLOCKS + OPEN_RANGE_TOLERANCE}（4.0 + 0.5 = 4.5，
     * server/src/world/container_open.rs）保持一致，两端都是欧氏距离，只要
     * 数值对齐即可——不对齐会出现 client 候选可交互、server 却拒绝的假交互带
     * （plan-bughunt-entity-interact-range-desync-v1）。
     */
    private static final double MAX_INTERACT_DISTANCE_BLOCKS = 4.5;
    private static final double MAX_INTERACT_DISTANCE_SQ =
        MAX_INTERACT_DISTANCE_BLOCKS * MAX_INTERACT_DISTANCE_BLOCKS;

    private ContainerOpenIntentSupport() {
    }

    static Optional<InteractCandidate> candidate(
        MinecraftClient client,
        String debugPrefix,
        Predicate<BongEntityModelKind> modelKindPredicate
    ) {
        EntityHitResult hit = entityHit(client);
        if (hit == null) {
            return Optional.empty();
        }
        if (!(hit.getEntity() instanceof BongModeledEntity modeled)) {
            return Optional.empty();
        }
        if (!modelKindPredicate.test(modeled.modelKind())) {
            return Optional.empty();
        }
        double distSq = client.player.squaredDistanceTo(hit.getEntity());
        if (!isWithinInteractRange(distSq)) {
            return Optional.empty();
        }
        return Optional.of(InteractCandidate.of(
            InteractIntent.OpenContainer,
            ReservedInteractionIntents.OPEN_CONTAINER_PRIORITY,
            distSq,
            debugPrefix + hit.getEntity().getId()
        ));
    }

    static boolean dispatch(MinecraftClient client, InteractCandidate candidate, String debugPrefix) {
        int candidateEntityId = candidateEntityId(candidate, debugPrefix);
        if (candidateEntityId < 0) {
            return false;
        }
        EntityHitResult hit = entityHit(client);
        if (hit == null || hit.getEntity().getId() != candidateEntityId) {
            return false;
        }
        ClientRequestSender.sendContainerOpen(candidateEntityId);
        return true;
    }

    static boolean isWithinInteractRange(double distSq) {
        return distSq <= MAX_INTERACT_DISTANCE_SQ;
    }

    static int candidateEntityId(InteractCandidate candidate, String debugPrefix) {
        if (candidate == null || candidate.debugLabel() == null) {
            return -1;
        }
        if (!candidate.debugLabel().startsWith(debugPrefix)) {
            return -1;
        }
        try {
            return Integer.parseInt(candidate.debugLabel().substring(debugPrefix.length()));
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
