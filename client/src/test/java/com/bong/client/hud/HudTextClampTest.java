package com.bong.client.hud;

import com.bong.client.state.NarrationState;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class HudTextClampTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @AfterEach
    void resetToastState() {
        BongToast.resetForTests();
    }

    @Test
    void clampAlphaBoundsToByteRange() {
        assertEquals(0, HudTextHelper.clampAlpha(-5));
        assertEquals(128, HudTextHelper.clampAlpha(128));
        assertEquals(255, HudTextHelper.clampAlpha(999));
    }

    @Test
    void withAlphaUsesClampedChannel() {
        assertEquals(0x00ABCDEF, HudTextHelper.withAlpha(0xABCDEF, -1));
        assertEquals(0xFFABCDEF, HudTextHelper.withAlpha(0xABCDEF, 999));
    }

    @Test
    void emptyChildRenderersRemainSafeNoOps() {
        List<HudRenderCommand> commands = new ArrayList<>();

        assertTrue(!ZoneHudRenderer.append(commands, ZoneState.empty(), FIXED_WIDTH, 120, 10, 22));
        assertTrue(!ToastHudRenderer.append(commands, 0L, FIXED_WIDTH, 120, 10, 34));
        assertTrue(!VisualHudRenderer.append(commands, VisualEffectState.none(), 0L, FIXED_WIDTH, 120, 320, 180));
        assertTrue(commands.isEmpty());
    }
}
