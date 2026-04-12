package com.bong.client.insight;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

/**
 * 监听 {@link InsightOfferStore}：
 * <ul>
 *   <li>有新 offer 推入 → 自动打开 InsightOfferScreen。</li>
 *   <li>offer 被清空 (玩家提交后 / 服务端撤回) → 关闭当前 screen。</li>
 *   <li>断线 → 重置 store。</li>
 * </ul>
 *
 * <p>同时注册调试键 J：在没有真实 offer 时强行弹出 mock 数据，便于美工/手感联调。
 */
public final class InsightOfferScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String DEBUG_KEY_TRANSLATION = "key.bong-client.debug_insight_offer";
    private static KeyBinding debugKey;

    private InsightOfferScreenBootstrap() {
    }

    public static void register() {
        debugKeyBinding();

        InsightOfferStore.addListener(InsightOfferScreenBootstrap::onStoreChanged);

        ClientTickEvents.END_CLIENT_TICK.register(InsightOfferScreenBootstrap::onEndClientTick);

        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) ->
            client.execute(InsightOfferStore::clearOnDisconnect));

        BongClient.LOGGER.info("Registered insight offer screen bootstrap (debug key: J)");
    }

    private static KeyBinding debugKeyBinding() {
        if (debugKey == null) {
            debugKey = KeyBindingHelper.registerKeyBinding(
                new KeyBinding(DEBUG_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_J, CATEGORY));
        }
        return debugKey;
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null) {
            return;
        }
        while (debugKeyBinding().wasPressed()) {
            if (InsightOfferStore.snapshot() == null) {
                InsightOfferStore.replace(MockInsightOfferData.firstInduceBreakthrough());
            }
        }
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

    /** 测试 hook：触发 mock 弹窗 (绕过 keybinding)。 */
    public static void debugTriggerMockOffer() {
        if (InsightOfferStore.snapshot() == null) {
            InsightOfferStore.replace(MockInsightOfferData.firstInduceBreakthrough());
        }
    }
}
