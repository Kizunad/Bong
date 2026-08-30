package com.bong.client.hud;

import com.bong.client.input.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.util.function.BooleanSupplier;
import java.util.function.LongSupplier;

public final class HudImmersionControls {
    private static final String CATEGORY = "category.bong-client";
    private static final String TOGGLE_KEY = "key.bong-client.hud_immersive_toggle";
    private static KeyBinding toggleKey;

    private HudImmersionControls() {
    }

    public static void register() {
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(HudImmersionControls::onEndClientTick);
    }

    private static void onEndClientTick(MinecraftClient client) {
        consumeInstalledTogglePresses(System::currentTimeMillis);
    }

    static int consumeInstalledTogglePresses(LongSupplier nowMillis) {
        return consumeTogglePresses(keyBinding()::wasPressed, nowMillis);
    }

    static int consumeTogglePresses(BooleanSupplier wasPressed, LongSupplier nowMillis) {
        int consumed = 0;
        while (wasPressed.getAsBoolean()) {
            HudImmersionMode.toggleManual(nowMillis.getAsLong());
            consumed++;
        }
        return consumed;
    }

    private static KeyBinding keyBinding() {
        if (toggleKey == null) {
            installToggleKey(BongKeybindRegistry.global());
        }
        return toggleKey;
    }

    static KeyBinding installToggleKey(BongKeybindRegistry registry) {
        // 默认不绑键，F1-F9 留给快捷槽；玩家可在控制设置中显式重绑。
        return toggleKey = registry.register(new BongKeybindRegistry.BindingSpec(
            new BongKeybindRegistry.BindingOwner("hud.immersive_toggle"),
            TOGGLE_KEY,
            InputUtil.Type.KEYSYM,
            InputUtil.UNKNOWN_KEY.getCode(),
            CATEGORY
        ));
    }

    static void resetControlsForTests() {
        toggleKey = null;
    }
}
