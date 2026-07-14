package com.bong.client.hud;

import com.bong.client.inventory.model.MorphEntry;
import com.bong.client.inventory.state.MorphStateStore;
import net.minecraft.client.MinecraftClient;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;

/**
 * plan-race-system-v1 PR-5b — 易形（{@code morph.yixing}）HUD 反馈：
 * <ul>
 *   <li>左下角形态图标——**仅在本地玩家处于易形态时显示**（{@link MorphStateStore}
 *       查不到本地玩家即隐藏，HUD 条件显示硬约束：未激活隐藏而非灰掉）。</li>
 *   <li>施法期白色 vignette（{@link MorphCastVignetteState}，opacity 峰值 0.15，
 *       fade-in 8t / fade-out 12t）。</li>
 * </ul>
 *
 * <p>图标资源 {@code skill_scroll_morph_yixing.png} 待 {@code /gen-image} 生成
 * （见仓库 [BLOCKED] 标注）——本 planner 先接线 + text fallback（显示形态名），
 * 图片到位后把 {@code text} 命令换成 {@code HudRenderCommand.texture}。
 */
public final class MorphHudPlanner {
    private static final int ICON_X = 10;
    private static final int ICON_Y_FROM_BOTTOM = 46;
    private static final int TEXT_COLOR = 0xFFE8DFC8;
    private static final int VIGNETTE_RGB = 0xFFFFFF;

    private MorphHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight, long nowMillis) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        double vignetteAlpha = MorphCastVignetteState.alphaAt(nowMillis);
        if (vignetteAlpha > 0.0) {
            out.add(HudRenderCommand.screenTint(
                HudRenderLayer.MORPH,
                HudTextHelper.withAlpha(VIGNETTE_RGB, (int) Math.round(vignetteAlpha * 255.0))
            ));
        }

        Integer localPlayerId = localPlayerEntityId();
        if (localPlayerId != null) {
            Optional<MorphEntry> morph = MorphStateStore.morphOf(localPlayerId);
            if (morph.isPresent()) {
                int y = Math.max(10, screenHeight - ICON_Y_FROM_BOTTOM);
                out.add(HudRenderCommand.text(
                    HudRenderLayer.MORPH,
                    "形态：" + formLabel(morph.get().formRaceId()),
                    ICON_X,
                    y,
                    TEXT_COLOR
                ));
            }
        }

        return List.copyOf(out);
    }

    private static String formLabel(String formRaceId) {
        if ("whale".equals(formRaceId)) {
            return "飞鲸";
        }
        return formRaceId == null || formRaceId.isBlank() ? "?" : formRaceId;
    }

    private static Integer localPlayerEntityId() {
        MinecraftClient client = MinecraftClient.getInstance();
        // 无头测试环境下 getInstance() 本身可能返回 null（未跑 Fabric client 引导）。
        if (client == null || client.player == null) {
            return null;
        }
        return client.player.getId();
    }
}
