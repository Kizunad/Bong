package com.bong.client.scroll;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

/**
 * 监听 {@link ScrollReadStore}（plan-scroll-reading-v1 P1，范式 A，仿
 * {@code InsightOfferScreenBootstrap}）：
 * <ul>
 *   <li>有新 ScrollOpen 推入 → 自动打开 {@link ScrollReadScreen}。</li>
 *   <li>store 被清空（玩家自己关屏 / 断线兜底）→ 若当前正显示阅读屏则关闭。</li>
 *   <li>断线 → 重置 store。</li>
 * </ul>
 */
public final class ScrollReadScreenBootstrap {
    private ScrollReadScreenBootstrap() {
    }

    public static void register() {
        ScrollReadStore.addListener(ScrollReadScreenBootstrap::onStoreChanged);

        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) ->
            client.execute(ScrollReadStore::clearOnDisconnect));

        BongClient.LOGGER.info("Registered scroll read screen bootstrap via store listener");
    }

    static void onStoreChanged(ScrollOpenViewModel offer) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) {
            return;
        }
        client.execute(() -> applyStoreChange(client, offer));
    }

    static void applyStoreChange(MinecraftClient client, ScrollOpenViewModel offer) {
        Screen current = client.currentScreen;
        if (offer == null) {
            // store 被清空：若当前正显示阅读屏，则关掉（走 setScreen 而非再次调用
            // ScrollReadStore.close()，避免和触发本次清空的路径重复回传 ScrollReadClosed）。
            if (current instanceof ScrollReadScreen) {
                client.setScreen(null);
            }
            return;
        }
        // 来了新 ScrollOpen：同一卷（scrollId 相同）不重建屏，避免打断玩家正在看的页；
        // 不同卷（或当前根本没开着阅读屏）则打开新屏。
        if (!(current instanceof ScrollReadScreen existing)
            || !existing.viewModel().scrollId().equals(offer.scrollId())) {
            client.setScreen(new ScrollReadScreen(offer));
        }
    }
}
