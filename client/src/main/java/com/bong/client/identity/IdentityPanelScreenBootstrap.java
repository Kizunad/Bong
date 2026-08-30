package com.bong.client.identity;

import com.bong.client.BongClient;
import com.bong.client.input.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

/** plan-identity-v1 P5：按 O 打开身份面板；server 侧继续校验灵龛与冷却。 */
public final class IdentityPanelScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_identity_panel";
    static final int DEFAULT_KEY = GLFW.GLFW_KEY_O;

    private static KeyBinding openScreenKey;

    private IdentityPanelScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(IdentityPanelScreenBootstrap::onEndClientTick);
        // plan-bughunt-client-identity-panel-stale-session-v1 — IdentityPanelScreen 是
        // 普通 vanilla Screen，ButtonWidget 的 onPress 回调在 init() 时把 identityId /
        // cooldown 固化进 lambda，不像 owo 组件能就地 refresh。面板已打开时若只靠 render()
        // 重画文字，新 snapshot 到达也改不动按钮，出现"文字新、按钮仍绑旧 identityId"的
        // split-brain（含断线清理触发的清空快照）。订阅 store，面板开着就整份重建。
        IdentityPanelStateStore.addListener(IdentityPanelScreenBootstrap::onStoreChanged);
        BongClient.LOGGER.info("Registered identity panel bootstrap keybinding on key: O");
    }

    static void onStoreChanged(IdentityPanelState state) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null) {
            return;
        }
        client.execute(() -> {
            if (client.currentScreen instanceof IdentityPanelScreen) {
                client.setScreen(new IdentityPanelScreen());
            }
        });
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }
        while (keyBinding().wasPressed()) {
            requestOpenScreen(client);
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = BongKeybindRegistry.global().register(
                new BongKeybindRegistry.BindingSpec(
                    new BongKeybindRegistry.BindingOwner("identity.open_panel"),
                    OPEN_KEY_TRANSLATION,
                    InputUtil.Type.KEYSYM,
                    DEFAULT_KEY,
                    CATEGORY
                )
            );
        }
        return openScreenKey;
    }

    private static void requestOpenScreen(MinecraftClient client) {
        client.execute(() -> {
            if (client.currentScreen instanceof IdentityPanelScreen) {
                return;
            }
            client.setScreen(new IdentityPanelScreen());
        });
    }
}
