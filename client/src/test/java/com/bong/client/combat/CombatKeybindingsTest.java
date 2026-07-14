package com.bong.client.combat;

import net.minecraft.client.option.KeyBinding;
import org.junit.jupiter.api.Test;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

class CombatKeybindingsTest {
    @Test
    void registersNineQuickSlotsAsF1ThroughF9() {
        List<KeyBinding> captured = new ArrayList<>();

        KeyBinding[] registered = CombatKeybindings.registerQuickSlotKeys(binding -> {
            captured.add(binding);
            return binding;
        });

        assertEquals(QuickSlotConfig.SLOT_COUNT, registered.length);
        assertEquals(9, captured.size());
        for (int slot = 0; slot < registered.length; slot++) {
            KeyBinding binding = captured.get(slot);
            assertSame(binding, registered[slot]);
            assertEquals("key.bong-client.quick_slot_" + (slot + 1), binding.getTranslationKey());
            assertEquals("category.bong-client.combat", binding.getCategory());
            assertEquals(GLFW.GLFW_KEY_F1 + slot, binding.getDefaultKey().getCode());
        }
    }
}
