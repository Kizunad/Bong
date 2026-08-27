package com.bong.client.cultivation.voidaction;

import com.bong.client.BongClient;
import com.bong.client.input.KeybindMigrationService;
import com.bong.client.ui.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

public final class VoidActionScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_void_action_screen";
    private static final InputUtil.Key LEGACY_DEFAULT_KEY =
        InputUtil.Type.KEYSYM.createFromCode(GLFW.GLFW_KEY_O);
    private static final String LEGACY_MIGRATION_ID = "void-action-open-screen-o-v1";
    private static KeybindMigrationService migrationService;
    private static KeyBinding openScreenKey;

    private VoidActionScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientLifecycleEvents.CLIENT_STARTED.register(VoidActionScreenBootstrap::migrateLegacyBinding);
        ClientTickEvents.END_CLIENT_TICK.register(VoidActionScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered void action screen bootstrap on key: UNKNOWN");
    }

    private static void migrateLegacyBinding(MinecraftClient client) {
        if (client == null || client.options == null) return;
        try {
            boolean migrated = migrationService().migrateOnce(
                LEGACY_MIGRATION_ID,
                () -> BongKeybindRegistry.global().migrateLegacyBoundKey(
                    OPEN_KEY_TRANSLATION,
                    LEGACY_DEFAULT_KEY,
                    InputUtil.UNKNOWN_KEY,
                    client.options::setKeyCode
                ),
                () -> {
                    client.options.setKeyCode(keyBinding(), LEGACY_DEFAULT_KEY);
                    KeyBinding.updateKeysByCode();
                }
            );
            if (migrated) {
                BongClient.LOGGER.info(
                    "Migrated legacy void action screen keybinding O to UNKNOWN; existing custom bindings were preserved."
                );
            }
        } catch (IllegalStateException exception) {
            BongClient.LOGGER.error(
                "Unable to persist the VoidAction keybinding migration marker; "
                    + "migration was rolled back and client startup will continue.",
                exception
            );
        }
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        while (keyBinding().wasPressed()) {
            client.execute(() -> {
                if (!(client.currentScreen instanceof VoidActionScreen)) {
                    client.setScreen(new VoidActionScreen());
                }
            });
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = BongKeybindRegistry.global().register(
                new BongKeybindRegistry.BindingSpec(
                    new BongKeybindRegistry.BindingOwner("void_action.open_screen"),
                    OPEN_KEY_TRANSLATION,
                    InputUtil.Type.KEYSYM,
                    InputUtil.UNKNOWN_KEY.getCode(),
                    CATEGORY
                )
            );
        }
        return openScreenKey;
    }

    private static KeybindMigrationService migrationService() {
        if (migrationService == null) {
            migrationService = KeybindMigrationService.clientConfig();
        }
        return migrationService;
    }
}
