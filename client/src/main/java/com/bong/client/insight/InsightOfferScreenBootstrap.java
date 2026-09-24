package com.bong.client.insight;

import com.bong.client.BongClient;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.sound.SoundEvents;
import net.minecraft.sound.SoundCategory;

/**
 * 监听 {@link InsightOfferStore}：
 * <ul>
 *   <li>有新 offer 推入 → 自动打开 InsightOfferScreen。</li>
 *   <li>offer 被清空 (玩家提交后 / 服务端撤回) → 关闭当前 screen。</li>
 *   <li>断线 → 重置 store。</li>
 * </ul>
 *
 * <p>身份规则（r7-insight-settlement.tsv）：current/pending 两槽都以 {@code offerId}
 * 识别实例；旧 offer 的迟到回调 (close/removal/decision) 不得清除后来 offer B。
 * 屏幕已安装时，若新 offer 与当前屏同实例则 no-op；不同实例先由 store 结算 outgoing
 * 本地终态，再替换屏幕。安装失败 (store 侧 pending 已满) 时保持 bounded pending 可重试。
 */
public final class InsightOfferScreenBootstrap {
    private InsightOfferScreenBootstrap() {
    }

    public static void register() {
        InsightOfferStore.addListener(InsightOfferScreenBootstrap::onStoreChanged);

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
        if (current instanceof InsightOfferScreen existing) {
            if (existing.offer().offerId().equals(offer.offerId())) {
                // 同实例刷新（store 保留 token）：不重开
                return;
            }
            // 不同实例：先由 store 结算 outgoing 本地终态（replace 已做），再开新屏
        }
        // 来了新邀约：打开屏幕 (即使当前已有别的屏，也覆盖之——顿悟是被动事件，应当抢焦点)
        // plan §P2 视听规格：顿悟开屏音效（区分「喜」vs 心魔「危」）。
        // block.beacon.activate pitch1.0/vol0.4 + entity.player.levelup pitch1.2/vol0.3
        if (client.player != null) {
            client.player.playSound(SoundEvents.BLOCK_BEACON_ACTIVATE, SoundCategory.PLAYERS, 0.4F, 1.0F);
            client.player.playSound(SoundEvents.ENTITY_PLAYER_LEVELUP, SoundCategory.PLAYERS, 0.3F, 1.2F);
        }
        client.setScreen(new InsightOfferScreen(offer));
    }

}
