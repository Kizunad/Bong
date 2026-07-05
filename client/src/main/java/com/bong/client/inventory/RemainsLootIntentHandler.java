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
    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        RemainsStore.Entry nearest = nearest(client);
        if (nearest == null) {
            return Optional.empty();
        }
        double distanceSq = distanceSq(client, nearest);
        return Optional.of(InteractCandidate.of(
            InteractIntent.LootRemains,
            ReservedInteractionIntents.LOOT_REMAINS_PRIORITY,
            distanceSq,
            "remains:" + nearest.remainsId()
        ));
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        RemainsStore.Entry nearest = nearest(client);
        if (nearest == null) {
            return false;
        }
        ClientRequestSender.sendRemainsLoot(nearest.remainsId());
        return true;
    }

    private static RemainsStore.Entry nearest(MinecraftClient client) {
        if (client == null || client.player == null) {
            return null;
        }
        return RemainsStore.nearestTo(
            client.player.getX(),
            client.player.getY(),
            client.player.getZ()
        );
    }

    private static double distanceSq(MinecraftClient client, RemainsStore.Entry entry) {
        double dx = client.player.getX() - entry.worldPosX();
        double dy = client.player.getY() - entry.worldPosY();
        double dz = client.player.getZ() - entry.worldPosZ();
        return dx * dx + dy * dy + dz * dz;
    }
}
