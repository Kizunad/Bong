package com.bong.client.forge;

import com.bong.client.BongClient;
import com.bong.client.ui.BongKeybindRegistry;
import net.fabricmc.loader.api.FabricLoader;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.nio.file.Path;

/** plan-forge-v1 §3.3 — 右键砧方块 / 按键打开锻炉 UI 的启动器。 */
public final class ForgeScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_forge_screen";
    private static final InputUtil.Key LEGACY_DEFAULT_KEY =
        InputUtil.Type.KEYSYM.createFromCode(GLFW.GLFW_KEY_U);
    private static final String LEGACY_MIGRATION_ID = "forge-open-screen-u-v1";
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
        boolean migrated = BongKeybindRegistry.global().migrateLegacyBoundKeyOnce(
            LEGACY_MIGRATION_ID,
            migrationMarkerFile(),
            OPEN_KEY_TRANSLATION,
            LEGACY_DEFAULT_KEY,
            InputUtil.UNKNOWN_KEY,
            client.options::setKeyCode
        );
        if (migrated) {
            BongClient.LOGGER.info(
                "Migrated legacy forge screen keybinding U to UNKNOWN; existing custom bindings were preserved."
            );
        }
    }

    private static Path migrationMarkerFile() {
        return FabricLoader.getInstance().getConfigDir()
            .resolve("bong-client-keybind-migrations.properties");
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        while (keyBinding().wasPressed()) {
            requestOpenForgeScreen(client);
        }
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
}
