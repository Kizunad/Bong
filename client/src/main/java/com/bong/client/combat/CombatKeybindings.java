package com.bong.client.combat;

import com.bong.client.BongClient;
import com.bong.client.botany.BotanyHudBootstrap;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;

import java.util.function.IntConsumer;

/**
 * Combat-HUD key bindings (§7). Registers F1-F9 quick-use keys, the Jiemai
 * reaction key (V, DefenseWindow only), and the R spell-volume hold.
 *
 * <p>Dispatch is wired through simple {@link IntConsumer} hooks so the
 * networking layer can plug in without pulling MC types into the combat
 * module.
 */
public final class CombatKeybindings {
    private static final String CATEGORY = "category.bong-client.combat";

    private static final KeyBinding[] QUICK_SLOT_KEYS = new KeyBinding[QuickSlotConfig.SLOT_COUNT];
    private static KeyBinding jiemaiKey;
    private static KeyBinding spellVolumeKey;
    private static KeyBinding eventStreamToggleKey;
    // plan-shield-block-v1 P1 — 举盾持键（默认未绑定，玩家可自定义；右键路径走 MixinMouse）。
    private static KeyBinding shieldHoldKey;

    private static volatile IntConsumer quickSlotHandler = slot -> { };
    private static volatile Runnable jiemaiHandler = () -> { };
    private static volatile SpellVolumeHoldHandler spellVolumeHandler = pressed -> { };
    private static volatile Runnable eventStreamToggleHandler = () -> { };
    // plan-shield-block-v1 P1 — 举盾 hold handler（和 spellVolumeHandler 同模式）。
    private static volatile ShieldHoldHandler shieldHoldHandler = pressed -> { };

    private static boolean spellVolumeHeldLastTick = false;
    // plan-shield-block-v1 P1 — 举盾 hold 状态（同 spellVolumeHeldLastTick 模式）。
    private static boolean shieldHeldLastTick = false;

    private CombatKeybindings() {
    }

    public static void register() {
        for (int i = 0; i < QuickSlotConfig.SLOT_COUNT; i++) {
            QUICK_SLOT_KEYS[i] = KeyBindingHelper.registerKeyBinding(new KeyBinding(
                "key.bong-client.quick_slot_" + (i + 1),
                InputUtil.Type.KEYSYM,
                GLFW.GLFW_KEY_F1 + i,
                CATEGORY
            ));
        }
        jiemaiKey = KeyBindingHelper.registerKeyBinding(new KeyBinding(
            "key.bong-client.jiemai_react",
            InputUtil.Type.KEYSYM,
            GLFW.GLFW_KEY_V,
            CATEGORY
        ));
        spellVolumeKey = KeyBindingHelper.registerKeyBinding(new KeyBinding(
            "key.bong-client.spell_volume_hold",
            InputUtil.Type.KEYSYM,
            GLFW.GLFW_KEY_R,
            CATEGORY
        ));
        eventStreamToggleKey = KeyBindingHelper.registerKeyBinding(new KeyBinding(
            "key.bong-client.event_stream_toggle",
            InputUtil.Type.KEYSYM,
            GLFW.GLFW_KEY_UNKNOWN,
            CATEGORY
        ));
        // plan-shield-block-v1 P1 — 举盾持键（默认未绑定；主要路径是右键 MixinMouse 仲裁）。
        shieldHoldKey = KeyBindingHelper.registerKeyBinding(new KeyBinding(
            "key.bong-client.shield_hold",
            InputUtil.Type.KEYSYM,
            GLFW.GLFW_KEY_UNKNOWN,
            CATEGORY
        ));

        ClientTickEvents.END_CLIENT_TICK.register(CombatKeybindings::onTick);
        BongClient.LOGGER.info("Registered combat HUD keybindings (F1-F9, V, R, event stream toggle, shield hold).");
    }

    public static void setQuickSlotHandler(IntConsumer handler) {
        quickSlotHandler = handler == null ? slot -> { } : handler;
    }

    public static void setJiemaiHandler(Runnable handler) {
        jiemaiHandler = handler == null ? () -> { } : handler;
    }

    public static void setSpellVolumeHoldHandler(SpellVolumeHoldHandler handler) {
        spellVolumeHandler = handler == null ? pressed -> { } : handler;
    }

    public static void setEventStreamToggleHandler(Runnable handler) {
        eventStreamToggleHandler = handler == null ? () -> { } : handler;
    }

    // plan-shield-block-v1 P1 — 举盾 hold handler setter。
    public static void setShieldHoldHandler(ShieldHoldHandler handler) {
        shieldHoldHandler = handler == null ? pressed -> { } : handler;
    }

    private static void onTick(MinecraftClient client) {
        if (client == null || client.player == null) return;

        for (int i = 0; i < QUICK_SLOT_KEYS.length; i++) {
            while (QUICK_SLOT_KEYS[i].wasPressed()) {
                quickSlotHandler.accept(i);
            }
        }

        while (jiemaiKey.wasPressed()) {
            jiemaiHandler.run();
        }

        while (eventStreamToggleKey.wasPressed()) {
            eventStreamToggleHandler.run();
        }

        // Spell-volume is hold-to-show: detect edge transitions via the
        // KeyBinding.isPressed() poll (wasPressed only fires on key-down).
        if (BotanyHudBootstrap.shouldCaptureSpellVolumeKey()) {
            if (spellVolumeHeldLastTick) {
                spellVolumeHandler.onSpellVolumeHold(false);
                spellVolumeHeldLastTick = false;
            }
        } else {
            boolean heldNow = spellVolumeKey.isPressed();
            if (heldNow != spellVolumeHeldLastTick) {
                spellVolumeHandler.onSpellVolumeHold(heldNow);
                spellVolumeHeldLastTick = heldNow;
            }
        }

        // plan-shield-block-v1 P1 — 举盾持键边沿检测（同 spellVolumeKey 模式）。
        // 主要路径是右键（MixinMouse）；此处提供备用键盘绑定（默认 UNKNOWN）。
        boolean shieldHeldNow = shieldHoldKey.isPressed();
        if (shieldHeldNow != shieldHeldLastTick) {
            shieldHoldHandler.onShieldHold(shieldHeldNow);
            shieldHeldLastTick = shieldHeldNow;
        }

    }

    @FunctionalInterface
    public interface SpellVolumeHoldHandler {
        void onSpellVolumeHold(boolean pressed);
    }

    // plan-shield-block-v1 P1 — 举盾 hold 回调接口（同 SpellVolumeHoldHandler 模式）。
    @FunctionalInterface
    public interface ShieldHoldHandler {
        /** @param pressed true=按下边沿(RaiseShield), false=松开边沿(LowerShield) */
        void onShieldHold(boolean pressed);
    }
}
