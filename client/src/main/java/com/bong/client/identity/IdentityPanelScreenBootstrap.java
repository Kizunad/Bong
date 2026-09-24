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

    /** 应用组合根：将生产状态源与命令 sink 注入 XML 屏幕。 */
    public static IdentityPanelScreen create() {
        return new IdentityPanelScreenFactory(
            IdentityPanelUiStateSource.production(),
            IdentityPanelClientIntentSink.production()
        ).create();
    }

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(IdentityPanelScreenBootstrap::onEndClientTick);
        // 断线清理或新快照到达时，重新创建 XML 宿主，确保按钮回调和显示状态来自同一份快照。
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
                client.setScreen(create());
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
            client.setScreen(create());
        });
    }
}
