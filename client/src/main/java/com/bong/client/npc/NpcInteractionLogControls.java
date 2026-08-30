package com.bong.client.npc;

import com.bong.client.input.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.util.function.BooleanSupplier;

public final class NpcInteractionLogControls {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String KEY_TRANSLATION = "key.bong-client.npc_interaction_log";
    private static KeyBinding key;
    private static boolean registered;

    private NpcInteractionLogControls() {
    }

    public static void register() {
        if (registered) {
            return;
        }
        installInteractionLogKey(BongKeybindRegistry.global());
        ClientTickEvents.END_CLIENT_TICK.register(NpcInteractionLogControls::onEndClientTick);
        registered = true;
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null) {
            return;
        }
        consumeInstalledTogglePresses(
            client.player != null,
            client.currentScreen != null
        );
    }

    static int consumeInstalledTogglePresses(boolean playerPresent, boolean screenOpen) {
        return consumeTogglePresses(
            playerPresent,
            screenOpen,
            () -> key != null && key.wasPressed()
        );
    }

    static int consumeTogglePresses(
        boolean playerPresent,
        boolean screenOpen,
        BooleanSupplier wasPressed
    ) {
        if (!playerPresent || screenOpen) {
            return 0;
        }
        int consumed = 0;
        while (wasPressed.getAsBoolean()) {
            NpcInteractionLogStore.toggleVisible();
            consumed++;
        }
        return consumed;
    }

    static KeyBinding installInteractionLogKey(BongKeybindRegistry registry) {
        // 默认不绑键，F1-F9 留给快捷槽；玩家可在控制设置中显式重绑。
        return key = registry.register(new BongKeybindRegistry.BindingSpec(
            new BongKeybindRegistry.BindingOwner("npc.interaction_log"),
            KEY_TRANSLATION,
            InputUtil.Type.KEYSYM,
            InputUtil.UNKNOWN_KEY.getCode(),
            CATEGORY
        ));
    }

    static void resetControlsForTests() {
        key = null;
        registered = false;
    }
}
