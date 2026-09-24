package com.bong.client.ui.window;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Set;
import static org.junit.jupiter.api.Assertions.*;

class WindowLayoutPreferenceStoreTest {
    @TempDir Path directory;

    @Test
    void diskRoundtripStoresLayoutWithoutPersistingBusinessIdentity() throws Exception {
        Path path = directory.resolve("window-layout.json");
        var store = new WindowLayoutPreferenceStore(path);
        var manager = new UiWindowManager(640, 360);
        var definition = new UiWindowDefinition("item-inspect", "item-inspect", 100, 100,
            Set.of(UiWindowDefinition.Capability.WINDOW));
        var state = manager.openOrFocus(definition, manager.key("item-inspect", "private-session-token"),
            new UiWindowManager.Rect(20, 30, 240, 200));
        manager.pin(state.key(), true);
        manager.minimize(state.key());
        store.remember(state);
        store.background("terrain");
        store.motion(false);
        var hud = new WindowLayoutPreferenceStore.HudPreference(.12, -.08, 1.25, false);
        store.hud("hud-mini_body", hud);
        store.save();
        var loaded = new WindowLayoutPreferenceStore(path);
        loaded.load();
        assertEquals(new WindowLayoutPreferenceStore.Preference(state.bounds(), true, true), loaded.window("item-inspect"));
        assertEquals("terrain", loaded.background());
        assertFalse(loaded.motion());
        assertEquals(hud, loaded.hud("hud-mini_body"), "HUD 显隐、比例和位移必须一起恢复");
        assertFalse(Files.readString(path).contains("private-session-token"),
            "本地偏好不得保存物品/工位/会话 identity");
        manager.reset();
        assertTrue(manager.snapshot().isEmpty(), "配置不能重建旧连接中的业务窗口");
        loaded.resetLayouts();
        loaded.save();
        store.load();
        assertEquals(WindowLayoutPreferenceStore.HudPreference.DEFAULT, store.hud("hud-mini_body"));
        assertNull(store.window("item-inspect"));
        assertEquals("terrain", store.background(), "还原布局不应改动背景偏好");
    }

    @Test
    void brokenConfigDoesNotPartiallyReplaceUsableDefaults() throws Exception {
        Path path = directory.resolve("window-layout.json");
        Files.writeString(path, "{\"version\":1,\"windows\":{},\"background\":\"terrain\",\"motion\":[]}");
        var store = new WindowLayoutPreferenceStore(path);
        assertThrows(java.io.IOException.class, store::load);
        assertEquals("cosmos", store.background());
        assertTrue(store.motion());
    }
}
