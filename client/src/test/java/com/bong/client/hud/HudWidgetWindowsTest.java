package com.bong.client.hud;

import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.ui.window.WindowLayoutPreferenceStore;
import org.junit.jupiter.api.Test;

import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class HudWidgetWindowsTest {
    @Test
    void hiddenWidgetIsFilteredButFullscreenEffectsRemainOnScreen() {
        var manager = new UiWindowManager(320, 240);
        var preferences = new WindowLayoutPreferenceStore(Path.of("window-layout-test.json"));
        var hud = new HudWidgetWindows(manager, preferences);
        var commands = List.of(
            HudRenderCommand.rect(HudRenderLayer.MINI_BODY, 20, 30, 40, 50, 0xFFFFFFFF),
            HudRenderCommand.screenTint(HudRenderLayer.MINI_BODY, 0x55000000),
            HudRenderCommand.texture(HudRenderLayer.MINI_BODY, "screen-mask.png", 0, 0, 320, 240, -1));
        hud.capture(commands, 320, 240, text -> text == null ? 0 : text.length() * 6);
        preferences.hud("hud-mini_body", preferences.hud("hud-mini_body").visible(false));

        var laidOut = hud.layout(commands);
        assertEquals(commands.subList(1, 3), laidOut, "隐藏面板必须保留同 layer 的全屏效果");
        assertTrue(laidOut.get(0).isScreenTint(), "全屏效果不属于可移动 HUD 窗口");
    }

    @Test
    void lockAndResetDoNotChangeDefaultGeometryOrBusinessScope() {
        var manager = new UiWindowManager(320, 240);
        var preferences = new WindowLayoutPreferenceStore(Path.of("window-layout-test.json"));
        var hud = new HudWidgetWindows(manager, preferences);
        hud.capture(List.of(HudRenderCommand.rect(HudRenderLayer.MINI_BODY, 20, 30, 40, 50, 0xFFFFFFFF)),
            320, 240, text -> 0);
        var state = hud.open(HudWidgetWindows.Widget.MINI_BODY);
        hud.remember(state);
        assertEquals(WindowLayoutPreferenceStore.HudPreference.DEFAULT, preferences.hud(state.key().windowType()));
        assertFalse(state.scope().isClosed(), "HUD 布局操作不能关闭窗口 scope");

        manager.resize(state.key(), "260", "120");
        hud.remember(state);
        assertNotEquals(1, preferences.hud(state.key().windowType()).scale(), "尺寸调整应形成 HUD 缩放偏好");
        manager.minimize(state.key());
        hud.remember(state);
        assertFalse(hud.visible(HudWidgetWindows.Widget.MINI_BODY));
        assertSame(state, hud.open(HudWidgetWindows.Widget.MINI_BODY));
        assertTrue(hud.visible(HudWidgetWindows.Widget.MINI_BODY), "从列表恢复最小化窗口须保留锁定状态");
        hud.resetLayouts();
        assertEquals(WindowLayoutPreferenceStore.HudPreference.DEFAULT, preferences.hud(state.key().windowType()));
        assertTrue(state.pinned());
        assertFalse(state.scope().isClosed());
    }
}
