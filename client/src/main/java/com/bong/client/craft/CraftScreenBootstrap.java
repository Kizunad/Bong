package com.bong.client.craft;

import com.bong.client.BongClient;
import com.bong.client.input.BongKeybindRegistry;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.ui.window.UiWindowRuntime;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

/** plan-craft-ux-v1 — C 键打开手搓台。 */
public final class CraftScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String OPEN_KEY_TRANSLATION = "key.bong-client.open_craft_screen";
    private static KeyBinding openScreenKey;
    private static boolean registered;

    private CraftScreenBootstrap() {}

    public static void register() {
        if (registered) {
            return;
        }
        keyBinding();
        ClientTickEvents.END_CLIENT_TICK.register(CraftScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered craft screen bootstrap keybinding on key: C");
        registered = true;
    }

    private static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) {
            return;
        }
        while (keyBinding().wasPressed()) {
            if (client.currentScreen == null) open(client, CraftContext.HANDCRAFT);
        }
    }

    /** 普通制作入口不能覆盖死亡、裁决或其他模态 Screen。 */
    static void open(MinecraftClient client, CraftContext context) {
        if (client.player == null || client.world == null
            || (client.currentScreen != null && !(client.currentScreen instanceof InspectScreen))) return;
        if (!(client.currentScreen instanceof InspectScreen)) {
            client.setScreen(new InspectScreen(InventoryStateStore.snapshot()));
        }
        UiWindowRuntime.openCraft(context);
    }

    public static boolean available(CraftContext context) {
        if (context.workbench() == null) return true;
        var client = MinecraftClient.getInstance();
        if (client.player == null || client.world == null) return false;
        var target = context.workbench();
        var entity = client.world.getEntityById(target.entityId());
        return entity instanceof BongModeledEntity modeled
            && WorkbenchInteractIntentHandler.isWorkbenchKind(modeled.modelKind()) && !entity.isRemoved()
            && entity.getBlockX() == target.x() && entity.getBlockY() == target.y() && entity.getBlockZ() == target.z()
            && WorkbenchInteractIntentHandler.isWithinInteractRange(client.player.getX(), client.player.getY(),
                client.player.getZ(), entity.getX(), entity.getY(), entity.getZ());
    }

    private static KeyBinding keyBinding() {
        if (openScreenKey == null) {
            openScreenKey = BongKeybindRegistry.global().register(
                new BongKeybindRegistry.BindingSpec(
                    new BongKeybindRegistry.BindingOwner("craft.open_screen"),
                    OPEN_KEY_TRANSLATION,
                    InputUtil.Type.KEYSYM,
                    GLFW.GLFW_KEY_C,
                    CATEGORY
                )
            );
        }
        return openScreenKey;
    }
}
