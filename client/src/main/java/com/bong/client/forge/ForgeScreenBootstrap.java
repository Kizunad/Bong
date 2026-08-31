package com.bong.client.forge;

import com.bong.client.BongClient;
import com.bong.client.combat.ForgeCarrierScreenBootstrap;
import com.bong.client.input.ClientInputPolicy;
import com.bong.client.input.KeybindMigrationService;
import com.bong.client.input.BongKeybindRegistry;
import com.bong.client.combat.screen.ForgeCarrierScreen;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

/** plan-forge-v1 §3.3 — 右键砧方块 / 按键打开锻炉 UI 的启动器。 */
public final class ForgeScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_forge_screen";
    private static final InputUtil.Key LEGACY_DEFAULT_KEY =
        InputUtil.Type.KEYSYM.createFromCode(GLFW.GLFW_KEY_U);
    private static final String LEGACY_MIGRATION_ID = "forge-open-screen-u-v1";
    private static KeybindMigrationService migrationService;
    private static KeyBinding openScreenKey;

    private ForgeScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientLifecycleEvents.CLIENT_STARTED.register(ForgeScreenBootstrap::migrateLegacyBinding);
        ClientTickEvents.END_CLIENT_TICK.register(ForgeScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info(
            "Registered forge screen bootstrap keybinding (default unbound; configure in controls)."
        );
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
                    "Migrated legacy forge screen keybinding U to UNKNOWN; existing custom bindings were preserved."
                );
            }
        } catch (IllegalStateException exception) {
            BongClient.LOGGER.error(
                "Unable to persist the Forge keybinding migration marker; "
                    + "migration was rolled back and client startup will continue.",
                exception
            );
        }
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        // Drain legacy U presses while extracting so a failed migration cannot replay Forge later.
        while (keyBinding().wasPressed()) {
            if (ClientInputPolicy.shouldDispatchForgeOpen()) {
                requestOpenForgeScreen(client);
            }
        }
    }

    private static KeybindMigrationService migrationService() {
        if (migrationService == null) {
            migrationService = KeybindMigrationService.clientConfig();
        }
        return migrationService;
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = BongKeybindRegistry.global().register(
                new BongKeybindRegistry.BindingSpec(
                    new BongKeybindRegistry.BindingOwner("forge.open_screen"),
                    OPEN_KEY_TRANSLATION,
                    InputUtil.Type.KEYSYM,
                    InputUtil.UNKNOWN_KEY.getCode(),
                    CATEGORY
                )
            );
        }
        return openScreenKey;
    }

    private static void requestOpenForgeScreen(MinecraftClient client) {
        client.execute(() -> {
            if (client.currentScreen instanceof ForgeScreen) {
                return;
            }
            client.setScreen(new ForgeScreen());
        });
    }

    /**
     * 由现有锻炉界面转入暗器注入界面；生产 sink 必须通过组合根组装。
     *
     * <p>该入口保持在 forge 应用层，避免 XML Screen 自行创建网络实现。</p>
     */
    static void requestOpenForgeCarrierScreen(MinecraftClient client) {
        if (client == null) {
            return;
        }
        client.execute(() -> {
            if (client.currentScreen instanceof ForgeCarrierScreen) {
                return;
            }
            client.setScreen(ForgeCarrierScreenBootstrap.create());
        });
    }
}
