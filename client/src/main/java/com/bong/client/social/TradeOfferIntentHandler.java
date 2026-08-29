package com.bong.client.social;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.inventory.state.InventoryStateStore;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.hit.EntityHitResult;

import java.util.Optional;

public final class TradeOfferIntentHandler implements IntentHandler {
    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        EntityHitResult hit = playerHit(client);
        if (hit == null || TradeOfferScreenViewModel.collectChoices(InventoryStateStore.snapshot()).isEmpty()) {
            return Optional.empty();
        }
        double distanceSq = client.player.squaredDistanceTo(hit.getEntity());
        return Optional.of(InteractCandidate.of(
            InteractIntent.TradePlayer,
            ReservedInteractionIntents.TRADE_PLAYER_PRIORITY,
            distanceSq,
            "trade_player:" + hit.getEntity().getId()
        ));
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        EntityHitResult hit = playerHit(client);
        if (hit == null) {
            return false;
        }
        // 默认交互没有明确 item instance_id，必须拒绝而不能自动选择排序后的第一件。
        return false;
    }

    /** 由显式 picker 调用；只有当前库存中存在该 instance_id 才发送交易请求。 */
    public boolean dispatchSelected(MinecraftClient client, InteractCandidate candidate, long instanceId) {
        EntityHitResult hit = playerHit(client);
        if (hit == null || candidate == null || candidate.intent() != InteractIntent.TradePlayer || instanceId <= 0L) {
            return false;
        }
        if (TradeOfferScreenViewModel.findChoice(InventoryStateStore.snapshot(), instanceId).isEmpty()) return false;
        return TradeOfferClientIntentSink.production().dispatch(new TradeOfferIntent.Request(
            "entity:" + hit.getEntity().getId(), instanceId
        )).kind() == com.bong.client.ui.intent.UiIntentResult.Kind.LOCAL_ACCEPTED;
    }

    private static EntityHitResult playerHit(MinecraftClient client) {
        if (client == null || client.player == null) {
            return null;
        }
        if (!(client.crosshairTarget instanceof EntityHitResult hit)) {
            return null;
        }
        if (!(hit.getEntity() instanceof PlayerEntity)) {
            return null;
        }
        return hit;
    }
}
