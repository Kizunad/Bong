package com.bong.client.npc;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudTextHelper;
import com.bong.client.state.PlayerStateViewModel;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class ThreatAssessmentBar {
    public static final int WIDTH = 80;
    public static final int HEIGHT = 6;
    static final int COLOR_LOW = 0xFF2B7A40;
    static final int COLOR_MID = 0xFF9B7A22;
    static final int COLOR_HIGH = 0xFF8F2E2E;
    static final int COLOR_BG = 0xCC101018;

    private ThreatAssessmentBar() {
    }

    public static boolean visibleFor(PlayerStateViewModel viewer, NpcMoodState mood) {
        return mood != null && realmTier(viewer == null ? "" : viewer.realm()) >= 2;
    }

    public static List<HudRenderCommand> buildCommands(
        NpcMoodState mood,
        PlayerStateViewModel viewer,
        int x,
        int y,
        int alpha,
        HudTextHelper.WidthMeasurer widthMeasurer
    ) {
        if (!visibleFor(viewer, mood) || alpha <= 0) {
            return List.of();
        }
        int fill = Math.max(0, Math.min(WIDTH, (int) Math.round(WIDTH * mood.threatLevel())));
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y, WIDTH, HEIGHT, withAlpha(COLOR_BG, alpha)));
        if (fill > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x, y, fill, HEIGHT, withAlpha(colorFor(mood.threatLevel()), alpha)));
        }
        String label = labelFor(mood.threatLevel());
        out.add(HudRenderCommand.text(HudRenderLayer.TARGET_INFO, label, x + WIDTH + 6, y - 3, withAlpha(0xFFECE8D8, alpha)));
        if (realmTier(viewer.realm()) >= 3 && mood.qiLevelHint() != null) {
            out.add(HudRenderCommand.text(HudRenderLayer.TARGET_INFO, "真元" + mood.qiLevelHint(), x, y + 9, withAlpha(0xFFAAA8A0, alpha)));
        }
        if (realmTier(viewer.realm()) >= 4 && mood.innerMonologue() != null) {
            String text = HudTextHelper.clipToWidth(mood.innerMonologue(), 132, widthMeasurer);
            if (!text.isEmpty()) {
                out.add(HudRenderCommand.text(HudRenderLayer.TARGET_INFO, text, x, y + 20, withAlpha(0xFFD7C48A, alpha)));
            }
        }
        if (mood.threatLevel() > 0.90) {
            out.addAll(shatterCommands(x, y, alpha));
        }
        return List.copyOf(out);
    }

    public static int colorFor(double threatLevel) {
        if (threatLevel < 0.30) {
            return COLOR_LOW;
        }
        if (threatLevel < 0.60) {
            return COLOR_MID;
        }
        return COLOR_HIGH;
    }

    public static String labelFor(double threatLevel) {
        if (threatLevel > 0.90) {
            return "已癫狂";
        }
        if (threatLevel >= 0.60) {
            return "杀意";
        }
        if (threatLevel >= 0.30) {
            return "警惕";
        }
        return "恭敬";
    }

    static List<HudRenderCommand> shatterCommands(int x, int y, int alpha) {
        int color = withAlpha(0xFFE9D28B, alpha);
        return List.of(
            HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x + 8, y - 3, 8, 2, color),
            HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x + 36, y + 7, 7, 2, color),
            HudRenderCommand.rect(HudRenderLayer.TARGET_INFO, x + 67, y - 2, 6, 2, color)
        );
    }

    private static int realmTier(String realm) {
        String normalized = realm == null ? "" : realm.trim().toLowerCase(Locale.ROOT);
        return switch (normalized) {
            case "醒灵", "awaken" -> 0;
            case "引气", "induce" -> 1;
            case "凝脉", "condense" -> 2;
            case "固元", "solidify" -> 3;
            case "通灵", "spirit" -> 4;
            case "化虚", "void" -> 5;
            default -> 0;
        };
    }

    private static int withAlpha(int color, int alpha) {
        return ((Math.max(0, Math.min(255, alpha)) << 24) | (color & 0x00FFFFFF));
    }
}
