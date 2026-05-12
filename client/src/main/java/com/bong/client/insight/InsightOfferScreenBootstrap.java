package com.bong.client.insight;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

/**
 * 监听 {@link InsightOfferStore}：
 * <ul>
 *   <li>有新 offer 推入 → 自动打开 InsightOfferScreen。</li>
 *   <li>offer 被清空 (玩家提交后 / 服务端撤回) → 关闭当前 screen。</li>
 *   <li>断线 → 重置 store。</li>
 * </ul>
 */
public final class InsightOfferScreenBootstrap {
    private InsightOfferScreenBootstrap() {
    }

    public static void register() {
        InsightOfferStore.addListener(InsightOfferScreenBootstrap::onStoreChanged);

        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) ->
            client.execute(InsightOfferStore::clearOnDisconnect));

        BongClient.LOGGER.info("Registered insight offer screen bootstrap via store listener");
    }

    static void onStoreChanged(InsightOfferViewModel offer) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) {
            return;
        }
        client.execute(() -> applyStoreChange(client, offer));
    }

    static void applyStoreChange(MinecraftClient client, InsightOfferViewModel offer) {
        Screen current = client.currentScreen;
        if (offer == null) {
            // store 被清空：若当前正显示 offer 屏，则关掉
            if (current instanceof InsightOfferScreen) {
                client.setScreen(null);
            }
            return;
        }
        // 来了新邀约：打开屏幕 (即使当前已有别的屏，也覆盖之——顿悟是被动事件，应当抢焦点)
        if (!(current instanceof InsightOfferScreen existing) || !existing.offer().triggerId().equals(offer.triggerId())) {
            client.setScreen(new InsightOfferScreen(offer));
        }
    }

}
