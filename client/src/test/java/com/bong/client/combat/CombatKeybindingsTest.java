package com.bong.client.combat;

import net.minecraft.client.option.KeyBinding;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;

class CombatKeybindingsTest {
    @BeforeEach
    void resetBeforeTest() {
        KeyBinding.unpressAll();
        CombatKeybindings.resetForTests();
    }

    @AfterEach
    void resetAfterTest() {
        KeyBinding.unpressAll();
        CombatKeybindings.resetForTests();
    }

    @Test
    void onlyNineQuickSlotDefinitionsOwnF1ThroughF9() {
        List<KeyBinding> definitions = new ArrayList<>();
        CombatKeybindings.installBindings(binding -> {
            definitions.add(binding);
            return binding;
        });

        assertEquals(QuickSlotConfig.SLOT_COUNT + 4, definitions.size(),
            "Combat 的快捷槽与四个辅助键必须全部经过同一 registrar");
        List<KeyBinding> reservedDefinitions = definitions.stream()
            .filter(CombatKeybindingsTest::usesReservedFunctionKey)
            .toList();
        assertEquals(QuickSlotConfig.SLOT_COUNT, reservedDefinitions.size());
        Set<String> reservedOwners = reservedDefinitions.stream()
            .map(KeyBinding::getTranslationKey)
            .collect(Collectors.toSet());
        assertEquals(expectedQuickSlotTranslations(), reservedOwners,
            "Combat 内只有 quick_slot_1..9 可以默认占用 F1-F9");

        assertDefaultKey(definitions, "key.bong-client.jiemai_react", GLFW.GLFW_KEY_UNKNOWN);
        assertDefaultKey(definitions, "key.bong-client.spell_volume_hold", GLFW.GLFW_KEY_R);
        assertDefaultKey(definitions, "key.bong-client.event_stream_toggle", GLFW.GLFW_KEY_UNKNOWN);
        assertDefaultKey(definitions, "key.bong-client.shield_hold", GLFW.GLFW_KEY_UNKNOWN);
    }

    @Test
    void registrarResultIsInstalledAndReadByQuickSlotConsumer() {
        Map<String, KeyBinding> installedByDefinition = new LinkedHashMap<>();
        CombatKeybindings.installBindings(definition -> {
            KeyBinding installed = new KeyBinding(
                "test.registered." + definition.getTranslationKey(),
                definition.getDefaultKey().getCategory(),
                definition.getDefaultKey().getCode(),
                definition.getCategory()
            );
            installedByDefinition.put(definition.getTranslationKey(), installed);
            return installed;
        });

        List<Integer> dispatchedSlots = new ArrayList<>();
        CombatKeybindings.setQuickSlotHandler(dispatchedSlots::add);
        KeyBinding installedBoundarySlot = installedByDefinition.get(
            "key.bong-client.quick_slot_9"
        );
        KeyBinding.onKeyPressed(installedBoundarySlot.getDefaultKey());

        assertEquals(1, CombatKeybindings.consumeQuickSlotPresses());
        assertEquals(List.of(QuickSlotConfig.SLOT_COUNT - 1), dispatchedSlots,
            "tick consumer 必须读取 registrar 返回并安装的同一 F9 绑定");
    }

    private static boolean usesReservedFunctionKey(KeyBinding binding) {
        int code = binding.getDefaultKey().getCode();
        return code >= GLFW.GLFW_KEY_F1 && code <= GLFW.GLFW_KEY_F9;
    }

    private static Set<String> expectedQuickSlotTranslations() {
        return Set.of(
            "key.bong-client.quick_slot_1",
            "key.bong-client.quick_slot_2",
            "key.bong-client.quick_slot_3",
            "key.bong-client.quick_slot_4",
            "key.bong-client.quick_slot_5",
            "key.bong-client.quick_slot_6",
            "key.bong-client.quick_slot_7",
            "key.bong-client.quick_slot_8",
            "key.bong-client.quick_slot_9"
        );
    }

    private static void assertDefaultKey(
        List<KeyBinding> definitions,
        String translationKey,
        int expectedCode
    ) {
        KeyBinding definition = definitions.stream()
            .filter(binding -> binding.getTranslationKey().equals(translationKey))
            .findFirst()
            .orElseThrow();
        assertEquals(expectedCode, definition.getDefaultKey().getCode());
    }
}
