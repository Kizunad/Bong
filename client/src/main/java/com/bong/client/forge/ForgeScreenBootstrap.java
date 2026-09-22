package com.bong.client.forge;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.forge.state.ForgeStationStore;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.ui.window.UiWindowRuntime;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.math.BlockPos;

/** 交互键请求经服务端授权后，打开真实炼器砧的窗口。 */
public final class ForgeScreenBootstrap {
    private ForgeScreenBootstrap() {}

    public static void open(ForgeStationStore.Snapshot station) {
        var client = MinecraftClient.getInstance();
        if (!available(station.pos())
            || (client.currentScreen != null && !(client.currentScreen instanceof InspectScreen))) return;
        if (!(client.currentScreen instanceof InspectScreen)) {
            client.setScreen(new InspectScreen(InventoryStateStore.snapshot()));
        }
        UiWindowRuntime.openForge(station.pos());
    }

    public static void openCarrier() {
        var client = MinecraftClient.getInstance();
        if (available(ForgeStationStore.snapshot().pos())) {
            UiWindowRuntime.cancelInput();
            client.setScreen(com.bong.client.combat.ForgeCarrierScreenBootstrap.create());
        }
    }

    public static boolean available(BlockPos pos) {
        return available(MinecraftClient.getInstance(), pos);
    }

    static boolean available(MinecraftClient client, BlockPos pos) {
        if (pos == null || client == null || client.player == null || client.world == null) return false;
        if (Math.abs(client.player.getX() - pos.getX()) > 3
            || Math.abs(client.player.getY() - pos.getY()) > 3
            || Math.abs(client.player.getZ() - pos.getZ()) > 3) return false;
        for (var entity : client.world.getEntities()) {
            if (entity instanceof BongModeledEntity modeled && !entity.isRemoved()
                && modeled.modelKind() == BongEntityModelKind.FORGE_STATION && entity.getBlockPos().equals(pos)) return true;
        }
        return false;
    }
}
