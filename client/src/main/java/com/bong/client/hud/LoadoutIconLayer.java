package com.bong.client.hud;

import com.bong.client.combat.SkillBarEntry;

import java.util.ArrayList;
import java.util.List;

public final class LoadoutIconLayer {
    private LoadoutIconLayer() {}

    public static List<HudRenderCommand> buildSkillIconCommands(
        SkillBarEntry entry,
        int x,
        int y,
        int size
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (entry == null || entry.iconTexture().isBlank()) {
            return out;
        }
        out.add(HudRenderCommand.texture(HudRenderLayer.QUICK_BAR, entry.iconTexture(), x, y, size, size, 0xFFFFFFFF));
        appendAnqiOverlay(out, entry.id(), x, y, size);
        return out;
    }

    private static void appendAnqiOverlay(List<HudRenderCommand> out, String skillId, int x, int y, int size) {
        if (skillId == null || !skillId.startsWith("anqi.")) return;
        int color = switch (skillId) {
            case "anqi.multi_shot" -> 0xFF79E6B2;
            case "anqi.soul_inject" -> 0xFF8CE6FF;
            case "anqi.armor_pierce" -> 0xFFFF6C6C;
            case "anqi.echo_fractal" -> 0xFFB9A7FF;
            default -> 0xFFE6D27A;
        };
        out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x, y, size, 1, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x, y + size - 1, size, 1, color));
        if ("anqi.echo_fractal".equals(skillId)) {
            out.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, x + size - 6, y + 2, 4, 4, 0x80FFFFFF));
        }
    }
}
