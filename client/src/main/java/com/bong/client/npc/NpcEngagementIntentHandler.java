package com.bong.client.npc;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.hit.EntityHitResult;

import java.util.Optional;

public final class NpcEngagementIntentHandler implements IntentHandler {
    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        EntityHitResult hit = entityHit(client);
        if (hit == null || NpcMetadataStore.get(hit.getEntity().getId()) == null) {
            return Optional.empty();
        }
        double distanceSq = client.player.squaredDistanceTo(hit.getEntity());
        return Optional.of(InteractCandidate.of(
            InteractIntent.TalkNpc,
            ReservedInteractionIntents.TALK_NPC_PRIORITY,
            distanceSq,
            "talk_npc:" + hit.getEntity().getId()
        ));
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        EntityHitResult hit = entityHit(client);
        if (hit == null) {
            return false;
        }
        NpcMetadata metadata = NpcMetadataStore.get(hit.getEntity().getId());
        if (metadata == null) {
            return false;
        }
        ClientRequestSender.sendNpcInspectRequest(metadata.entityId());
        client.setScreen(new NpcDialogueScreen(metadata));
        return true;
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
