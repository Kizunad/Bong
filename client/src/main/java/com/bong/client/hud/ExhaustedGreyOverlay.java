package com.bong.client.hud;

import com.bong.client.combat.store.StatusEffectStore;

import java.util.ArrayList;
import java.util.List;

/**
 * 全力一击「虚脱」灰幕 + 时长条。
 *
 * <p>虚脱已从独立的 {@code full_power_exhausted_state} 路径收敛到标准 debuff
 * （{@code StatusEffectKind::Exhausted}，id {@code "exhausted"}），由 status_snapshot
 * 全量替换驱动。因此本 overlay 改读 {@link StatusEffectStore}：
 * /reset 或自然到期时服务器下发的空/缩短 status_snapshot 会立即清空/更新此条目，
 * 灰幕与时长条随之消失——根治旧路径「server 删组件、client 虚脱条赖着不走」的脱钩。
 */
public final class ExhaustedGreyOverlay {
    public static final String EXHAUSTED_EFFECT_ID = "exhausted";
    public static final int VIGNETTE_COLOR = 0x886E6A76;
    public static final int BAR_WIDTH = 120;
    public static final int BAR_HEIGHT = 3;
    public static final int TRACK_COLOR = 0x80303036;
    public static final int FILL_COLOR = 0xFFD0CCD8;

    private ExhaustedGreyOverlay() {}

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight, long nowMs) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }
        StatusEffectStore.Effect exhausted = findExhausted();
        if (exhausted == null || exhausted.remainingMs() <= 0L) {
            return out;
        }
        // 时长条比例：以服务器下发的剩余时长相对一个软上限（10s）归一，仅作视觉表现。
        // status_snapshot 每次都带最新 remaining_ms，到期即从快照消失，无需本地推算 recovery tick。
        long remainingMs = exhausted.remainingMs();
        double ratio = Math.max(0.0, Math.min(1.0, remainingMs / 10_000.0));
        int x = (screenWidth - BAR_WIDTH) / 2;
        int y = screenHeight - 88;
        out.add(HudRenderCommand.edgeVignette(HudRenderLayer.VISUAL, VIGNETTE_COLOR));
        out.add(HudRenderCommand.rect(HudRenderLayer.CAST_BAR, x, y, BAR_WIDTH, BAR_HEIGHT, TRACK_COLOR));
        out.add(HudRenderCommand.rect(
            HudRenderLayer.CAST_BAR,
            x,
            y,
            Math.max(1, (int) Math.round(BAR_WIDTH * ratio)),
            BAR_HEIGHT,
            FILL_COLOR
        ));
        out.add(HudRenderCommand.text(
            HudRenderLayer.CAST_BAR,
            "虚脱 " + Math.max(1L, remainingMs / 1000L) + "s",
            x,
            y - 10,
            FILL_COLOR
        ));
        return out;
    }

    /** 从标准 debuff 快照里找虚脱条目（id == "exhausted"）；无则返回 null。 */
    private static StatusEffectStore.Effect findExhausted() {
        for (StatusEffectStore.Effect effect : StatusEffectStore.snapshot()) {
            if (effect != null && EXHAUSTED_EFFECT_ID.equals(effect.id())) {
                return effect;
            }
        }
        return null;
    }
}
