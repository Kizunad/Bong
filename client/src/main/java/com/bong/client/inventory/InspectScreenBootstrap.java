package com.bong.client.inventory;

import com.bong.client.BongClient;
import com.bong.client.cultivation.QiColorObservedStore;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import net.minecraft.util.hit.EntityHitResult;

public final class InspectScreenBootstrap {
    private InspectScreenBootstrap() {}

    public static void register() {
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

    /**
     * 由应用层组装养护界面的生产依赖，避免 InspectScreen 直接依赖网络设施。
     */
    static void openRepairScreen(MinecraftClient client, com.bong.client.inventory.model.InventoryItem item) {
        if (client == null || item == null || item.instanceId() == 0L) return;
        int sx = 0;
        int sy = 64;
        int sz = 0;
        if (client.player != null) {
            sx = (int) Math.floor(client.player.getX());
            sy = (int) Math.floor(client.player.getY());
            sz = (int) Math.floor(client.player.getZ());
        }
        client.setScreen(com.bong.client.combat.screen.RepairScreenFactory.production().create(
            item.displayName(), (float) item.durability(), item.instanceId(), sx, sy, sz));
    }
}
