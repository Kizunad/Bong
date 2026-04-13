package com.bong.client.debug;

import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.Test;

import java.util.Arrays;
import java.util.List;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 验证 /vfx 命令暴露的 effect 列表随 {@link VisualEffectState.EffectType} 自动扩展，
 * 以后新增 EffectType 无需改命令代码。
 */
public class BongVfxCommandTest {

    @Test
    void availableEffectNamesExcludeNoneSentinel() {
        List<String> names = BongVfxCommand.availableEffectNames();
        assertFalse(names.isEmpty(), "至少应有一个可用效果");
        assertFalse(names.contains(VisualEffectState.EffectType.NONE.wireName()),
            "NONE 是哨兵值，不应出现在可触发列表中");
    }

    @Test
    void availableEffectNamesCoverAllNonNoneEnumValues() {
        List<String> expected = Arrays.stream(VisualEffectState.EffectType.values())
            .filter(t -> t != VisualEffectState.EffectType.NONE)
            .map(VisualEffectState.EffectType::wireName)
            .collect(Collectors.toList());

        List<String> actual = BongVfxCommand.availableEffectNames();

        assertEquals(expected.size(), actual.size(), "与 EffectType 值数量（去 NONE）保持一致");
        assertTrue(actual.containsAll(expected), "应覆盖全部 EffectType.wireName()");
    }

    @Test
    void availableEffectNamesIncludeStep1NewOverlayEffects() {
        List<String> names = BongVfxCommand.availableEffectNames();
        assertTrue(names.contains("blood_moon"));
        assertTrue(names.contains("demonic_fog"));
        assertTrue(names.contains("enlightenment_flash"));
        assertTrue(names.contains("tribulation_pressure"));
    }
}
