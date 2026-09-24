package com.bong.client.identity;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

final class IdentityPanelScreenTest {
    @Test
    void formatsSlashCommandsWithoutLeadingSlashForClientNetworkHandler() {
        assertEquals("identity switch 2", IdentityPanelScreen.switchIdentityCommand(2));
        assertEquals("identity switch 0", IdentityPanelScreen.switchIdentityCommand(-1));
        assertEquals("identity new 夜行 人", IdentityPanelScreen.newIdentityCommand("  夜行   人  "));
        assertEquals("identity rename 白面", IdentityPanelScreen.renameIdentityCommand("白面"));
        assertEquals("", IdentityPanelScreen.newIdentityCommand("   "));
    }

    @Test
    void formatsIdentityRowsWithActiveAndFrozenMarkers() {
        IdentityPanelEntry active = new IdentityPanelEntry(1, "白面", 0, false, List.of());
        IdentityPanelEntry frozen = new IdentityPanelEntry(0, "旧名", -80, true, List.of("dugu_revealed"));

        assertEquals("* #1 白面", IdentityPanelScreen.formatEntryLine(active, 1));
        assertEquals("  #0 旧名 [冷藏]", IdentityPanelScreen.formatEntryLine(frozen, 1));
    }

    @Test
    void typedIntentNormalizesNamesAndRejectsBlankValues() {
        assertEquals("夜行 人", new IdentityPanelIntent.NewIdentity("  夜行   人  ").name());
        assertEquals("白面", new IdentityPanelIntent.RenameIdentity(" 白面 ").name());
        assertThrows(IllegalArgumentException.class,
            () -> new IdentityPanelIntent.NewIdentity("   "));
        assertThrows(IllegalArgumentException.class,
            () -> new IdentityPanelIntent.RenameIdentity(null));
        assertEquals(0, new IdentityPanelIntent.SwitchIdentity(-1).identityId());
    }

    @Test
    void refreshAndCreateActionsKeepLifecycleAndCooldownGuards() throws IOException {
        String source = Files.readString(Path.of(
            "src/main/java/com/bong/client/identity/IdentityPanelScreen.java"));

        assertTrue(source.contains("scope.runIfOpen(() -> refresh(next))"),
            "排队的状态刷新必须经过 scope.runIfOpen，屏幕 removed() 后不得访问 XML 组件");
        assertTrue(source.contains("newButton.active(state.cooldownPassed())"),
            "新建按钮必须随身份切换冷却状态同步 active，避免发送必失败命令");
    }
}
