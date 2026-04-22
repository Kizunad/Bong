package com.bong.client.inventory;

import com.bong.client.BongClient;
import com.bong.client.combat.TreasureEquippedStore;
import com.bong.client.combat.WeaponEquippedStore;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

public final class InspectScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_inspect_screen";
    private static KeyBinding openScreenKey;

    private InspectScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(InspectScreenBootstrap::onEndClientTick);
        // 不在 JOIN 时清空 store —— 否则会与网络线程并发处理的 inventory_snapshot
        // 形成竞态：JOIN callback 经 client.execute 排队到主线程，期间快照已经到达
        // 并写入 store；queued task 一执行就把刚到的权威数据 reset 回 loading 态。
        // disconnect 已经清空，重新连接前 store 是 empty / revision=-1，不需要再清。
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) ->
            client.execute(InspectScreenBootstrap::clearInventorySnapshot)
        );
        BongClient.LOGGER.info("Registered inspect screen bootstrap keybinding on key: I");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        while (keyBinding().wasPressed()) {
            requestOpenInspectScreen(client);
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = KeyBindingHelper.registerKeyBinding(
                new KeyBinding(OPEN_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_I, CATEGORY)
            );
        }
        return openScreenKey;
    }

    /** plan-weapon-v1 §4.4：Mixin 拦截 E 键后调用。 */
    public static void openInspectScreen(MinecraftClient client) {
        requestOpenInspectScreen(client);
    }

    private static void requestOpenInspectScreen(MinecraftClient client) {
        client.execute(() -> {
            if (!shouldOpenInspectScreen(client.currentScreen)) {
                return;
            }

            InspectScreen screen = createScreenForCurrentState();
            if (screen == null) {
                BongClient.LOGGER.info("Rejecting inspect screen open: inventory loading");
                if (client.player != null) {
                    client.player.sendMessage(Text.literal("背包数据加载中…"), true);
                }
                return;
            }

            client.setScreen(screen);
        });
    }

    static boolean shouldOpenInspectScreen(Screen currentScreen) {
        return !(currentScreen instanceof InspectScreen);
    }

    static void clearInventorySnapshot() {
        InventoryStateStore.clearOnDisconnect();
        WeaponEquippedStore.clearOnDisconnect();
        TreasureEquippedStore.clearOnDisconnect();
    }

    static InspectScreen createScreenForCurrentState() {
        if (!InventoryStateStore.isAuthoritativeLoaded()) {
            return null;
        }

        return createScreen(InventoryStateStore.snapshot());
    }

    static InspectScreen createScreen(InventoryModel snapshot) {
        return new InspectScreen(snapshot);
    }
}
