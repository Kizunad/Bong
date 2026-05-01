package com.bong.client.social;

import com.bong.client.BongClient;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.hit.HitResult;
import net.minecraft.util.math.BlockPos;
import org.lwjgl.glfw.GLFW;

public final class SpiritNicheRevealBootstrap {
    private static final String CATEGORY = "category.bong-client.social";
    private static final String MARK_KEY_TRANSLATION = "key.bong-client.spirit_niche_mark_coordinate";
    private static final int GAZE_TICKS = 60;

    private static KeyBinding markKey;
    private static BlockPos focusedPos;
    private static int focusedTicks;
    private static BlockPos lastGazeSentPos;

    private SpiritNicheRevealBootstrap() {}

    public static void register() {
        markKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(MARK_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_M, CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(SpiritNicheRevealBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered spirit niche reveal interactions: gaze 3s, mark key M");
    }

    static void resetForTests() {
        focusedPos = null;
        focusedTicks = 0;
        lastGazeSentPos = null;
    }

    static boolean observeBlockForTests(BlockPos pos) {
        return observeBlock(pos);
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;

        BlockPos target = targetedBlock(client);
        if (target == null) {
            resetFocus();
        } else if (observeBlock(target)) {
            ClientRequestSender.sendSpiritNicheGaze(target.getX(), target.getY(), target.getZ());
        }

        while (markKey.wasPressed()) {
            if (target != null) {
                ClientRequestSender.sendSpiritNicheMarkCoordinate(target.getX(), target.getY(), target.getZ());
            }
        }
    }

    private static boolean observeBlock(BlockPos pos) {
        if (!pos.equals(focusedPos)) {
            focusedPos = pos;
            focusedTicks = 1;
            lastGazeSentPos = null;
            return false;
        }
        focusedTicks++;
        if (focusedTicks < GAZE_TICKS || pos.equals(lastGazeSentPos)) {
            return false;
        }
        lastGazeSentPos = pos;
        return true;
    }

    private static void resetFocus() {
        focusedPos = null;
        focusedTicks = 0;
        lastGazeSentPos = null;
    }

    private static BlockPos targetedBlock(MinecraftClient client) {
        if (client.crosshairTarget instanceof BlockHitResult hit
            && hit.getType() == HitResult.Type.BLOCK) {
            return hit.getBlockPos();
        }
        return null;
    }
}
