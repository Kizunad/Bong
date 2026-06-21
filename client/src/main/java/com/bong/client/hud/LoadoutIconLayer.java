package com.bong.client.hud;

import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillIconIds;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Predicate;

public final class LoadoutIconLayer {
    private LoadoutIconLayer() {}

    public static List<HudRenderCommand> buildSkillIconCommands(
        SkillBarEntry entry,
        int x,
        int y,
        int size
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        String iconTexture = entry == null ? null : entry.iconTexture();
        if (iconTexture == null || iconTexture.isBlank()) {
            return out;
        }
        out.add(HudRenderCommand.texture(HudRenderLayer.QUICK_BAR, iconTexture, x, y, size, size, 0xFFFFFFFF));
        appendAnqiOverlay(out, entry.id(), x, y, size);
        return out;
    }

    /**
     * 存在性感知版：只在贴图真存在时才画图标，否则返回空 → 调用方走文字标签兜底（不画紫黑 missing-texture）。
     *
     * @param textureExists 贴图存在性谓词（生产用 {@code HudTextureProbe::exists}，测试可注入桩）
     */
    public static List<HudRenderCommand> buildSkillIconCommands(
        SkillBarEntry entry,
        int x,
        int y,
        int size,
        Predicate<String> textureExists
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        String resolved = resolveExistingSkillTexture(entry, textureExists);
        if (resolved == null) {
            return out;
        }
        out.add(HudRenderCommand.texture(HudRenderLayer.QUICK_BAR, resolved, x, y, size, size, 0xFFFFFFFF));
        appendAnqiOverlay(out, entry.id(), x, y, size);
        return out;
    }

    /**
     * 解析技能图标贴图，按优先级取第一个【真实存在】的：
     * <ol>
     *   <li>服务端下发的 {@code iconTexture}（若非空且存在）</li>
     *   <li>按 id 拼的 {@code skill_scroll_<id>}（与查看界面同一路径，若存在）</li>
     * </ol>
     * 都不存在 → 返回 null（调用方走文字标签）。绝不返回一个画不出的路径。
     */
    static String resolveExistingSkillTexture(SkillBarEntry entry, Predicate<String> textureExists) {
        if (entry == null || textureExists == null) {
            return null;
        }
        String icon = entry.iconTexture();
        if (icon != null && !icon.isBlank() && textureExists.test(icon)) {
            return icon;
        }
        String candidate = SkillIconIds.scrollTexturePath(entry.id());
        return textureExists.test(candidate) ? candidate : null;
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
