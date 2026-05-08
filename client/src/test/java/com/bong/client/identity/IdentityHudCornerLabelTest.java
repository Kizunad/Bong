package com.bong.client.identity;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudTextHelper;

final class IdentityHudCornerLabelTest {
    @AfterEach
    void cleanup() {
        IdentityPanelStateStore.resetForTest();
    }

    @Test
    void emptyStateProducesNoCommands() {
        List<HudRenderCommand> cmds = IdentityHudCornerLabel.buildCommands(label -> 0, 320);
        assertTrue(cmds.isEmpty(), "no identity → no label");
    }

    @Test
    void labelFormatActiveIdentity() {
        String label =
                IdentityHudCornerLabel.formatLabel(
                        new IdentityPanelEntry(0, "kiz", 0, false, List.of()));
        assertEquals("[#0] kiz", label);
    }

    @Test
    void labelFormatFrozenIdentityIncludesMarker() {
        String label =
                IdentityHudCornerLabel.formatLabel(
                        new IdentityPanelEntry(1, "alt", -50, true, List.of("dugu_revealed")));
        assertEquals("[#1] alt [冷藏]", label);
    }

    @Test
    void buildCommandsRendersActiveIdentity() {
        IdentityPanelStateStore.replace(
                new IdentityPanelState(
                        0,
                        100L,
                        0L,
                        List.of(new IdentityPanelEntry(0, "kiz", 0, false, List.of()))));
        HudTextHelper.WidthMeasurer measurer = text -> text.length() * 6;
        List<HudRenderCommand> cmds = IdentityHudCornerLabel.buildCommands(measurer, 320);
        assertEquals(1, cmds.size(), "expected one render command");
    }

    @Test
    void buildCommandsReturnsEmptyWhenActiveIdNotInList() {
        IdentityPanelStateStore.replace(
                new IdentityPanelState(
                        99,
                        100L,
                        0L,
                        List.of(new IdentityPanelEntry(0, "kiz", 0, false, List.of()))));
        HudTextHelper.WidthMeasurer measurer = text -> text.length() * 6;
        List<HudRenderCommand> cmds = IdentityHudCornerLabel.buildCommands(measurer, 320);
        assertTrue(cmds.isEmpty(), "active id 99 不在列表 → no command");
    }

    @Test
    void cooldownPassedReflectsRemaining() {
        IdentityPanelState passed =
                new IdentityPanelState(
                        0, 0L, 0L, List.of(new IdentityPanelEntry(0, "kiz", 0, false, List.of())));
        assertTrue(passed.cooldownPassed());

        IdentityPanelState pending =
                new IdentityPanelState(
                        0,
                        0L,
                        12_000L,
                        List.of(new IdentityPanelEntry(0, "kiz", 0, false, List.of())));
        assertTrue(!pending.cooldownPassed());
    }

    @Test
    void storeReplaceNotifiesListeners() {
        final boolean[] called = {false};
        IdentityPanelStateStore.addListener(state -> called[0] = true);
        IdentityPanelStateStore.replace(IdentityPanelState.empty());
        assertTrue(called[0], "listener should be invoked");
    }
}
