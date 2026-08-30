package com.bong.client.lingtian;

import com.bong.client.BongClient;
import com.bong.client.input.KeybindMigrationService;
import com.bong.client.input.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.hit.HitResult;
import net.minecraft.util.math.BlockPos;
import org.lwjgl.glfw.GLFW;

/**
 * plan-lingtian-v1 §1.2-§1.7 — 按 L 打开 {@link LingtianActionScreen}。
 * 打开前 snapshot 当前 crosshair 指向的方块坐标作为目标 plot（crosshair 未命中方块时传 null，
 * Screen 内显示提示）。
 */
public final class LingtianActionScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_lingtian_action_screen";
    private static final InputUtil.Key LEGACY_DEFAULT_KEY =
        InputUtil.Type.KEYSYM.createFromCode(GLFW.GLFW_KEY_L);
    private static final String LEGACY_MIGRATION_ID = "lingtian-open-l-v1";
    private static final KeybindMigrationService MIGRATION_SERVICE =
        KeybindMigrationService.clientConfig();
    private static KeyBinding openScreenKey;

    private LingtianActionScreenBootstrap() {}

    public static void register() {
        keyBinding();
        ClientLifecycleEvents.CLIENT_STARTED.register(LingtianActionScreenBootstrap::migrateLegacyBinding);
        ClientTickEvents.END_CLIENT_TICK.register(LingtianActionScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info(
            "Registered lingtian action screen bootstrap (default unbound; configure in controls)."
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
                    "Migrated legacy lingtian action screen keybinding L to UNKNOWN; "
                        + "existing custom bindings were preserved."
                );
            }
        } catch (IllegalStateException exception) {
            BongClient.LOGGER.error(
                "Unable to persist the lingtian keybinding migration marker; "
                    + "migration was rolled back and client startup will continue.",
                exception
            );
        }
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        while (keyBinding().wasPressed()) {
            requestOpenScreen(client);
        }
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = BongKeybindRegistry.global().register(
                new BongKeybindRegistry.BindingSpec(
                    new BongKeybindRegistry.BindingOwner("lingtian.open_action_screen"),
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
            if (client.currentScreen instanceof LingtianActionScreen) {
                return;
            }
            BlockPos target = snapshotCrosshairBlockPos(client);
            client.setScreen(new LingtianActionScreen(target));
        });
    }

    /** 读玩家 crosshair，若命中方块则返回 pos；否则返回 null。 */
    private static BlockPos snapshotCrosshairBlockPos(MinecraftClient client) {
        HitResult hit = client.crosshairTarget;
        if (hit instanceof BlockHitResult bh && hit.getType() == HitResult.Type.BLOCK) {
            return bh.getBlockPos();
        }
        return null;
    }
}
