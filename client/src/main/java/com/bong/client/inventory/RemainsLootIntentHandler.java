package com.bong.client.inventory;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.inventory.state.RemainsStore;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;

import java.util.Optional;

/**
 * plan-remains-suite P0 — 遗骸 G 键统一交互（对应右键 {@code InteractEntityEvent} 路径）。
 * 照 {@link DroppedItemPickupIntentHandler} 的形状：候选按最近距离从
 * {@link RemainsStore} 挑（不做准星 raycast——遗骸和地面掉落物一样是"走近了就能捡"，
 * 真正的距离/layer/dimension 权威校验在 server 端 `handle_remains_loot_intents` 做）。
 */
public final class RemainsLootIntentHandler implements IntentHandler {
    static final String DEBUG_LABEL_PREFIX = "remains:";

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        if (client == null || client.player == null) {
            return Optional.empty();
        }
        return candidateAt(client.player.getX(), client.player.getY(), client.player.getZ());
    }

    static Optional<InteractCandidate> candidateAt(double x, double y, double z) {
        RemainsStore.Entry nearest = RemainsStore.nearestTo(x, y, z);
        if (nearest == null) {
            return Optional.empty();
        }
        double distanceSq = distanceSq(x, y, z, nearest);
        return Optional.of(InteractCandidate.of(
            InteractIntent.LootRemains,
            ReservedInteractionIntents.LOOT_REMAINS_PRIORITY,
            distanceSq,
            DEBUG_LABEL_PREFIX + nearest.remainsId()
        ));
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        String remainsId = remainsIdFromCandidate(candidate);
        if (remainsId == null) {
            return false;
        }
        ClientRequestSender.sendRemainsLoot(remainsId);
        return true;
    }

    static String remainsIdFromCandidate(InteractCandidate candidate) {
        if (candidate == null || candidate.intent() != InteractIntent.LootRemains) {
            return null;
        }
        String label = candidate.debugLabel();
        if (!label.startsWith(DEBUG_LABEL_PREFIX)) {
            return null;
        }
        String remainsId = label.substring(DEBUG_LABEL_PREFIX.length());
        return remainsId.isBlank() ? null : remainsId;
    }

    private static double distanceSq(double x, double y, double z, RemainsStore.Entry entry) {
        double dx = x - entry.worldPosX();
        double dy = y - entry.worldPosY();
        double dz = z - entry.worldPosZ();
        return dx * dx + dy * dy + dz * dz;
    }
}
