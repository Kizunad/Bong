package com.bong.client.tsy;

import com.bong.client.BongClient;
import com.bong.client.hud.SearchHudState;
import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.input.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

/**
 * plan-tsy-search-cancel-v1 §8.1 #1 — TSY 容器搜刮主动取消按键。
 *
 * <p>专属按键，不复用统一交互键 {@code G}（{@code InteractKeyRouter}）——玩家
 * 开始搜刮那一刻 {@code TsyContainerView.interactable()} 就会因
 * {@code searched_by} 非空而返回 {@code false}，router 候选不会再命中该容器，
 * 无法在不侵入路由优先级的前提下用"再按 G"表达取消。镜像
 * {@link ExtractInteractionBootstrap} 的既有模式：直接查
 * {@link SearchHudStateStore} 状态，绕过 router。
 */
public final class SearchCancelInteractionBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String CANCEL_KEY_TRANSLATION = "key.bong-client.tsy_search_cancel";
    private static KeyBinding cancelKey;
    private static boolean registered;

    private SearchCancelInteractionBootstrap() {
    }

    public static void register() {
        if (registered) {
            return;
        }
        cancelKey = BongKeybindRegistry.global().register(
            new BongKeybindRegistry.BindingSpec(
                new BongKeybindRegistry.BindingOwner("tsy.search_cancel"),
                CANCEL_KEY_TRANSLATION,
                InputUtil.Type.KEYSYM,
                GLFW.GLFW_KEY_H,
                CATEGORY
            )
        );
        ClientTickEvents.END_CLIENT_TICK.register(SearchCancelInteractionBootstrap::onTick);
        BongClient.LOGGER.info("Registered TSY search cancel keybinding on key: H");
        registered = true;
    }

    private static void onTick(MinecraftClient client) {
        if (client == null || client.player == null || client.options == null) {
            return;
        }
        while (cancelKey.wasPressed() && SearchHudStateStore.snapshot().phase() == SearchHudState.Phase.SEARCHING) {
            ClientRequestSender.sendCancelSearch();
        }
    }
}
