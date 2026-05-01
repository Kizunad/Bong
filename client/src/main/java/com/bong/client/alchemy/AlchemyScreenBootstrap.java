package com.bong.client.alchemy;

import com.bong.client.BongClient;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.hit.HitResult;
import net.minecraft.util.math.BlockPos;
import org.lwjgl.glfw.GLFW;

public final class AlchemyScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_alchemy_screen";
    private static KeyBinding openScreenKey;

    private AlchemyScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(AlchemyScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered alchemy screen bootstrap keybinding on key: K");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        while (keyBinding().wasPressed()) {
            requestOpenAlchemyScreen(client);
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = KeyBindingHelper.registerKeyBinding(
                new KeyBinding(OPEN_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_K, CATEGORY)
            );
        }
        return openScreenKey;
    }

    private static void requestOpenAlchemyScreen(MinecraftClient client) {
        client.execute(() -> {
            if (client.currentScreen instanceof AlchemyScreen) {
                return;
            }
            BlockPos pos = client.crosshairTarget instanceof BlockHitResult hit && hit.getType() == HitResult.Type.BLOCK
                ? hit.getBlockPos()
                : new BlockPos(0, 64, 0);
            com.bong.client.network.ClientRequestSender.sendAlchemyOpenFurnace(pos);
            client.setScreen(new AlchemyScreen(pos));
        });
    }
}
