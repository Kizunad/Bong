package com.bong.client.craft;

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
 * 制作台 marker 实体右键交互 handler。
 *
 * <p>制作台是 Bong 自定义 Marker + 自定义渲染实体，不能依赖 vanilla
 * InteractEntityEvent。客户端从准星命中的 {@link BongModeledEntity} 取 protocol
 * entity id，再发送 {@code workbench_open} C2S 交给 server 校验距离和打开 UI。</p>
 */
public final class WorkbenchInteractIntentHandler implements IntentHandler {
    private static final double MAX_INTERACT_DISTANCE_SQ = 5.0 * 5.0;
    private static final String LABEL_PREFIX = "workbench:";

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        EntityHitResult hit = entityHit(client);
        if (hit == null) {
            return Optional.empty();
        }
        if (!(hit.getEntity() instanceof BongModeledEntity modeled)) {
            return Optional.empty();
        }
        if (!isWorkbenchKind(modeled.modelKind())) {
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
            LABEL_PREFIX + hit.getEntity().getId()
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
        ClientRequestSender.sendWorkbenchOpen(candidateEntityId);
        return true;
    }

    public static boolean isWorkbenchKind(BongEntityModelKind kind) {
        return kind == BongEntityModelKind.WORKBENCH;
    }

    static int candidateEntityId(InteractCandidate candidate) {
        if (candidate == null || candidate.debugLabel() == null) {
            return -1;
        }
        if (!candidate.debugLabel().startsWith(LABEL_PREFIX)) {
            return -1;
        }
        try {
            return Integer.parseInt(candidate.debugLabel().substring(LABEL_PREFIX.length()));
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
