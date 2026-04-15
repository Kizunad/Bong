package com.bong.client.hud;

import com.bong.client.combat.SpellVolumeState;

import java.util.ArrayList;
import java.util.List;

/**
 * Bottom-right spell-volume scrub panel (§3.1). Shown only while the player
 * holds a spell weapon AND the R key (the state store is toggled by the
 * {@code CastSpellIntent} bootstrap).
 */
public final class SpellVolumeHudPlanner {
    public static final int PANEL_WIDTH = 180;
    public static final int PANEL_HEIGHT = 70;
    public static final int PANEL_BG = 0xC0101820;
    public static final int PANEL_BORDER = 0xFF80B0FF;
    public static final int TRACK_BG = 0xFF303848;
    public static final int RADIUS_COLOR = 0xFF80B0FF;
    public static final int VELOCITY_COLOR = 0xFFB0FF80;
    public static final int QI_COLOR = 0xFF40D0D0;

    public static final int RIGHT_MARGIN = 16;
    public static final int BOTTOM_MARGIN = 80;

    private SpellVolumeHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        SpellVolumeState state,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || !state.visible()) return out;
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        int x = screenWidth - PANEL_WIDTH - RIGHT_MARGIN;
        int y = screenHeight - PANEL_HEIGHT - BOTTOM_MARGIN;

        // Background
        out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, x, y, PANEL_WIDTH, PANEL_HEIGHT, PANEL_BG));
        // Border (1px rectangle outline)
        out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, x, y, PANEL_WIDTH, 1, PANEL_BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, x, y + PANEL_HEIGHT - 1, PANEL_WIDTH, 1, PANEL_BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, x, y, 1, PANEL_HEIGHT, PANEL_BORDER));
        out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, x + PANEL_WIDTH - 1, y, 1, PANEL_HEIGHT, PANEL_BORDER));

        int trackW = PANEL_WIDTH - 16;
        int trackX = x + 8;

        // Radius
        int trackY = y + 10;
        out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, trackX, trackY, trackW, 4, TRACK_BG));
        int rw = Math.round(normalize(state.radius(), SpellVolumeState.MIN_RADIUS, SpellVolumeState.MAX_RADIUS) * trackW);
        if (rw > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, trackX, trackY, rw, 4, RADIUS_COLOR));
        }

        // Velocity cap
        trackY = y + 30;
        out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, trackX, trackY, trackW, 4, TRACK_BG));
        int vw = Math.round(normalize(state.velocityCap(), SpellVolumeState.MIN_VELOCITY, SpellVolumeState.MAX_VELOCITY) * trackW);
        if (vw > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, trackX, trackY, vw, 4, VELOCITY_COLOR));
        }

        // Qi invest
        trackY = y + 50;
        out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, trackX, trackY, trackW, 4, TRACK_BG));
        int qw = Math.round(state.qiInvest() * trackW);
        if (qw > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.SPELL_VOLUME, trackX, trackY, qw, 4, QI_COLOR));
        }

        return out;
    }

    private static float normalize(float v, float min, float max) {
        if (max <= min) return 0.0f;
        return Math.max(0.0f, Math.min(1.0f, (v - min) / (max - min)));
    }
}
