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
        QiColorObservedStore.beginInspection();
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
