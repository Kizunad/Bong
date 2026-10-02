package com.bong.client.alchemy;

import com.bong.client.BongClient;
import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.ui.window.UiWindowRuntime;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.math.BlockPos;
import net.minecraft.world.World;

public final class AlchemyScreenBootstrap {
    private AlchemyScreenBootstrap() {}

    public static void register() {
        net.fabricmc.fabric.api.client.message.v1.ClientReceiveMessageEvents.ALLOW_GAME.register((message, overlay) -> {
            if (overlay || !(MinecraftClient.getInstance().currentScreen instanceof InspectScreen)) return true;
            String plain = net.minecraft.util.Formatting.strip(message.getString());
            if (plain == null || !plain.startsWith("[炼丹]")) return true;
            return !UiWindowRuntime.acceptAlchemyMessage(plain.substring("[炼丹]".length()).strip());
        });
        BongClient.LOGGER.info("Registered alchemy screen bootstrap via unified interaction key");
    }

    public static void requestOpenAlchemyScreen(MinecraftClient client, BlockPos pos) {
        if (client == null || pos == null) {
            return;
        }
        var connection = client.getNetworkHandler();
        var world = client.world;
        client.execute(() -> {
            if (connection == null || connection != client.getNetworkHandler() || world != client.world
                || !available(client, pos) || (client.currentScreen != null && !(client.currentScreen instanceof InspectScreen))) return;
            if (!(client.currentScreen instanceof InspectScreen)) {
                client.setScreen(new InspectScreen(InventoryStateStore.snapshot()));
            }
            UiWindowRuntime.openAlchemy(pos);
            com.bong.client.network.ClientRequestSender.sendAlchemyOpenFurnace(pos);
        });
    }

    public static boolean available(BlockPos pos) {
        return available(MinecraftClient.getInstance(), pos);
    }

    public static boolean available(MinecraftClient client, BlockPos pos) {
        if (client == null || pos == null || client.player == null || client.world == null
            || !client.world.getRegistryKey().equals(World.OVERWORLD)
            // 与服务端 within_reach / NEARBY_INTERACT 相同：炉底中心六格。
            || client.player.squaredDistanceTo(pos.getX() + .5, pos.getY(), pos.getZ() + .5) > 36) {
            return false;
        }
        for (var entity : client.world.getEntities()) {
            if (entity instanceof BongModeledEntity modeled
                && modeled.modelKind() == BongEntityModelKind.ALCHEMY_FURNACE
                && entity.getBlockPos().equals(pos)
                && !entity.isRemoved()) return true;
        }
        return false;
    }
}
