package com.bong.client.scroll;

import com.bong.client.BongClient;
import com.bong.client.ui.ScreenTransitionController;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

import java.util.function.Consumer;

/**
 * 监听 {@link ScrollReadStore}（plan-scroll-reading-v1 P1，范式 A，仿
 * {@code InsightOfferScreenBootstrap}）：
 * <ul>
 *   <li>有新 ScrollOpen 推入 → 自动打开 {@link ScrollReadScreen}。</li>
 *   <li>store 被清空（玩家自己关屏 / 断线兜底）→ 若当前正显示阅读屏则关闭。</li>
 *   <li>断线 → 同步失活阅读会话身份；数据快照仍由集中 lifecycle registry 清理。</li>
 * </ul>
 */
public final class ScrollReadScreenBootstrap {
    private ScrollReadScreenBootstrap() {
    }

    public static void register() {
        ScrollReadStore.addSessionListener(ScrollReadScreenBootstrap::onStoreChanged);

        // Invalidate only the atomic UI identity before any already queued open task can run.
        // The token-gated central disconnect path owns the later production data clear.
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) -> onDisconnect());

        BongClient.LOGGER.info("Registered scroll read screen bootstrap via store listener");
    }

    static void onStoreChanged(ScrollReadStore.ActiveSession session) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) {
            return;
        }
        client.execute(() -> applyStoreChange(client, session));
    }

    /**
     * 立即切断已排队 UI 任务对旧阅读会话的所有权；不在此处清 Store 数据。
     *
     * <p>{@code BongNetworkHandler} 的 handler-token gate 成功后会由 registry 调用
     * {@link ScrollReadStore#clearOnDisconnect()}。分离两者可避免 bootstrap 和 registry
     * 对同一 production Store 重复 clear。
     */
    static void onDisconnect() {
        ScrollReadStore.invalidateSessionIdentityOnDisconnect();
    }

    static void applyStoreChange(MinecraftClient client, ScrollReadStore.ActiveSession session) {
        applyStoreChange(
            client.currentScreen,
            ScreenTransitionController.pendingScreen(),
            client::setScreen,
            session
        );
    }

    static void applyStoreChange(
        Screen current,
        Screen pending,
        Consumer<Screen> setScreen,
        ScrollReadStore.ActiveSession session
    ) {
        if (!ScrollReadStore.isCurrent(session)) {
            return;
        }
        ScrollOpenViewModel offer = session == null ? null : session.viewModel();
        if (offer == null) {
            if (pending instanceof ScrollReadScreen) {
                ScreenTransitionController.cancelPendingOpen(pending);
            }
            // store 被清空：若当前正显示阅读屏，则关掉（走 setScreen 而非再次调用
            // ScrollReadStore.close()，避免和触发本次清空的路径重复回传 ScrollReadClosed）。
            if (current instanceof ScrollReadScreen) {
                setScreen.accept(null);
            }
            return;
        }
        // 同一会话的 refresh 不重建当前或 pending screen，避免打断玩家正在看的页；
        // 经历空态后的同卷会换 token，必须打开新 screen，不能复用旧会话身份。
        if (!belongsToSession(current, session) && !belongsToSession(pending, session)) {
            setScreen.accept(new ScrollReadScreen(offer, session.token()));
        }
    }

    static boolean belongsToSession(Screen screen, ScrollReadStore.ActiveSession session) {
        return screen instanceof ScrollReadScreen scrollScreen
            && session != null
            && scrollScreen.ownsSession(session.token());
    }
}
