package com.bong.client.identity;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

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
}
