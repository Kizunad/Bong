package com.bong.client.hud;

import com.bong.client.state.ZoneState;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongZoneHudTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @Test
    void buildCommandsIncludesCenteredFadeAndPersistentOverlay() {
        ZoneState zoneState = ZoneState.create("blood_valley", "Blood Valley", 0.42, 3, 1_000L);

        List<HudRenderCommand> commands = BongZoneHud.buildCommands(zoneState, 1_250L, FIXED_WIDTH, 220, 10, 22, 320, 180);

        assertEquals(2, commands.size());
        HudRenderCommand titleCommand = commands.get(0);
        HudRenderCommand overlayCommand = commands.get(1);
        assertEquals(HudRenderLayer.ZONE, titleCommand.layer());
        assertEquals("— Blood Valley —", titleCommand.text());
        assertEquals(HudTextHelper.withAlpha(BongZoneHud.TITLE_COLOR, 255), titleCommand.color());
        assertEquals(60, titleCommand.y());

        assertEquals(HudRenderLayer.ZONE, overlayCommand.layer());
        assertEquals("区域Blood Valley 灵气[████░░░░░░] 危☠☠☠", overlayCommand.text());
        assertEquals(10, overlayCommand.x());
        assertEquals(22, overlayCommand.y());
    }

    @Test
    void fadeAlphaDropsDuringFinalHalfSecondAndExpiresAtTwoSeconds() {
        assertEquals(255, BongZoneHud.centeredTitleAlpha(1_000L, 2_500L));
        assertEquals(128, BongZoneHud.centeredTitleAlpha(1_000L, 2_750L));
        assertEquals(0, BongZoneHud.centeredTitleAlpha(1_000L, 3_000L));
    }

    @Test
    void qiBarAndDangerTextClampSafely() {
        assertEquals("[██████████]", BongZoneHud.qiBar(5.0));
        assertEquals("[░░░░░░░░░░]", BongZoneHud.qiBar(-1.0));
        assertEquals("☠☠☠☠☠", BongZoneHud.dangerSymbols(99));
        assertEquals("无", BongZoneHud.dangerText(0));
    }

    @Test
    void narrowWidthKeepsOverlayButCanDropOversizedTitle() {
        ZoneState zoneState = ZoneState.create(
            "jade_valley",
            "Ancient Jade Valley of Unending Mist and Starfall Echoes",
            0.8,
            5,
            0L
        );

        List<HudRenderCommand> commands = BongZoneHud.buildCommands(zoneState, 500L, FIXED_WIDTH, 120, 10, 22, 10, 180);

        assertFalse(commands.isEmpty());
        assertTrue(commands.get(commands.size() - 1).text().contains("区域"));
    }
}
