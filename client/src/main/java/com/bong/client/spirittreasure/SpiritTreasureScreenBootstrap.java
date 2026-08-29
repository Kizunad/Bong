package com.bong.client.spirittreasure;

import com.bong.client.BongClient;
import com.bong.client.input.KeybindMigrationService;
import com.bong.client.ui.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class SpiritTreasureScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_spirit_treasure_screen";
    private static final InputUtil.Key LEGACY_DEFAULT_KEY =
        InputUtil.Type.KEYSYM.createFromCode(GLFW.GLFW_KEY_T);
    private static final String LEGACY_MIGRATION_ID = "spirit-treasure-open-t-v1";
    private static final KeybindMigrationService MIGRATION_SERVICE =
        KeybindMigrationService.clientConfig();

    private static KeyBinding openScreenKey;

    private SpiritTreasureScreenBootstrap() {
    }

    public static void register() {
        keyBinding();
        ClientLifecycleEvents.CLIENT_STARTED.register(SpiritTreasureScreenBootstrap::migrateLegacyBinding);
        ClientTickEvents.END_CLIENT_TICK.register(SpiritTreasureScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info(
            "Registered spirit treasure screen bootstrap keybinding (default unbound; configure in controls)."
        );
    }

    private static void migrateLegacyBinding(MinecraftClient client) {
        if (client == null || client.options == null) return;
        try {
            boolean migrated = MIGRATION_SERVICE.migrateOnce(
                LEGACY_MIGRATION_ID,
                () -> {
                    if (!keyBinding().matchesKey(LEGACY_DEFAULT_KEY.getCode(), 0)) {
                        return false;
                    }
                    client.options.setKeyCode(keyBinding(), InputUtil.UNKNOWN_KEY);
                    KeyBinding.updateKeysByCode();
                    return true;
                },
                () -> {
                    client.options.setKeyCode(keyBinding(), LEGACY_DEFAULT_KEY);
                    KeyBinding.updateKeysByCode();
                }
            );
            if (migrated) {
                BongClient.LOGGER.info(
                    "Migrated legacy spirit treasure screen keybinding T to UNKNOWN; "
                        + "existing custom bindings were preserved."
                );
            }
        } catch (IllegalStateException exception) {
            BongClient.LOGGER.error(
                "Unable to persist the spirit treasure keybinding migration marker; "
                    + "migration was rolled back and client startup will continue.",
                exception
            );
        }
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }
        while (keyBinding().wasPressed()) {
            requestOpenScreen(client);
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = BongKeybindRegistry.global().register(
                new BongKeybindRegistry.BindingSpec(
                    new BongKeybindRegistry.BindingOwner("spirittreasure.open_screen"),
                    OPEN_KEY_TRANSLATION,
                    InputUtil.Type.KEYSYM,
                    InputUtil.UNKNOWN_KEY.getCode(),
                    CATEGORY
                )
            );
        }
        return openScreenKey;
    }

    private static void requestOpenScreen(MinecraftClient client) {
        client.execute(() -> {
            if (client.currentScreen instanceof SpiritTreasureScreen) {
                return;
            }
            client.setScreen(new SpiritTreasureScreen());
        });
    }
}
