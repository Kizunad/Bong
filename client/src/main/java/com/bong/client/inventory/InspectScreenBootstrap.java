package com.bong.client.inventory;

import com.bong.client.BongClient;
import com.bong.client.combat.TreasureEquippedStore;
import com.bong.client.combat.WeaponEquippedStore;
import com.bong.client.cultivation.QiColorObservedStore;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import net.minecraft.util.hit.EntityHitResult;

public final class InspectScreenBootstrap {
    private InspectScreenBootstrap() {}

    public static void register() {
        // 不在 JOIN 时清空 store —— 否则会与网络线程并发处理的 inventory_snapshot
        // 形成竞态：JOIN callback 经 client.execute 排队到主线程，期间快照已经到达
        // 并写入 store；queued task 一执行就把刚到的权威数据 reset 回 loading 态。
        // disconnect 已经清空，重新连接前 store 是 empty / revision=-1，不需要再清。
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) ->
            client.execute(InspectScreenBootstrap::clearInventorySnapshot)
        );
        BongClient.LOGGER.info("Registered inspect screen bootstrap via vanilla E inventory interception");
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

            requestQiColorInspectForCrosshairTarget(client);
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
        QiColorObservedStore.clear();
    }

    static void requestQiColorInspectForCrosshairTarget(MinecraftClient client) {
        QiColorObservedStore.clear();
        String target = crosshairEntityTarget(client);
        if (target != null) {
            ClientRequestSender.sendQiColorInspect(target);
        }
    }

    static String crosshairEntityTarget(MinecraftClient client) {
        if (client == null || client.player == null || !(client.crosshairTarget instanceof EntityHitResult hit)) {
            return null;
        }
        if (hit.getEntity() == client.player) {
            return null;
        }
        return "entity:" + hit.getEntity().getId();
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
