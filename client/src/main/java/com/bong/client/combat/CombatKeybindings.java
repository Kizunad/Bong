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
 * reaction key (V, DefenseWindow only), the R spell-volume hold, and the three
 * unbound defense-stance toggles.
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
    private static KeyBinding switchStanceJiemai;
    private static KeyBinding switchStanceTishi;
    private static KeyBinding switchStanceJueling;

    private static volatile IntConsumer quickSlotHandler = slot -> { };
    private static volatile Runnable jiemaiHandler = () -> { };
    private static volatile SpellVolumeHoldHandler spellVolumeHandler = pressed -> { };
    private static volatile StanceHandler stanceHandler = stance -> { };

    private static boolean spellVolumeHeldLastTick = false;

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
        // Unbound by default — the player binds these in MC settings (§7.3).
        switchStanceJiemai = KeyBindingHelper.registerKeyBinding(new KeyBinding(
            "key.bong-client.stance_jiemai",
            InputUtil.Type.KEYSYM,
            InputUtil.UNKNOWN_KEY.getCode(),
            CATEGORY
        ));
        switchStanceTishi = KeyBindingHelper.registerKeyBinding(new KeyBinding(
            "key.bong-client.stance_tishi",
            InputUtil.Type.KEYSYM,
            InputUtil.UNKNOWN_KEY.getCode(),
            CATEGORY
        ));
        switchStanceJueling = KeyBindingHelper.registerKeyBinding(new KeyBinding(
            "key.bong-client.stance_jueling",
            InputUtil.Type.KEYSYM,
            InputUtil.UNKNOWN_KEY.getCode(),
            CATEGORY
        ));

        ClientTickEvents.END_CLIENT_TICK.register(CombatKeybindings::onTick);
        BongClient.LOGGER.info("Registered combat HUD keybindings (F1-F9, V, R, stance triad).");
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

    public static void setStanceHandler(StanceHandler handler) {
        stanceHandler = handler == null ? stance -> { } : handler;
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

        while (switchStanceJiemai.wasPressed()) {
            stanceHandler.onStanceSwitch(DefenseStanceState.Stance.JIEMAI);
        }
        while (switchStanceTishi.wasPressed()) {
            stanceHandler.onStanceSwitch(DefenseStanceState.Stance.TISHI);
        }
        while (switchStanceJueling.wasPressed()) {
            stanceHandler.onStanceSwitch(DefenseStanceState.Stance.JUELING);
        }
    }

    @FunctionalInterface
    public interface SpellVolumeHoldHandler {
        void onSpellVolumeHold(boolean pressed);
    }

    @FunctionalInterface
    public interface StanceHandler {
        void onStanceSwitch(DefenseStanceState.Stance stance);
    }
}
