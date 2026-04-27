package com.bong.client.hud;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-tsy-container-v1 §5.2 — TSY 容器搜刮 HUD planner。
 *
 * <p>渲染规则：
 * <ul>
 *   <li>SEARCHING：屏幕底部中央渲染进度条 + 文字「正在搜刮：&lt;kind&gt; [&lt;s&gt;s]」</li>
 *   <li>COMPLETED_FLASH：进度条满格 + 黄绿色提示文字</li>
 *   <li>ABORTED_FLASH：红色文字「搜刮中断：&lt;原因&gt;」</li>
 *   <li>IDLE：不渲染</li>
 * </ul>
 *
 * <p>纯函数：给定 state + 屏幕尺寸 → 出 RenderCommand 列表。无副作用，便于单测。
 */
public final class SearchProgressHudPlanner {
    public static final int BAR_WIDTH = 140;
    public static final int BAR_HEIGHT = 4;
    public static final int BOTTOM_MARGIN = 88;
    public static final int TEXT_OFFSET_Y = 6;

    public static final int TRACK_COLOR = 0xC0202830;
    public static final int FILL_COLOR = 0xFF8FB050;
    public static final int COMPLETED_COLOR = 0xFFFFD060;
    public static final int ABORTED_COLOR = 0xFFE05030;
    public static final int TEXT_COLOR = 0xFFE0E0E0;

    private SearchProgressHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        SearchHudState state,
        int screenWidth,
        int screenHeight
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (state == null || state.phase() == SearchHudState.Phase.IDLE) {
            return out;
        }
        if (screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }

        int barX = (screenWidth - BAR_WIDTH) / 2;
        int barY = screenHeight - BAR_HEIGHT - BOTTOM_MARGIN;

        switch (state.phase()) {
            case SEARCHING -> {
                out.add(HudRenderCommand.rect(
                    HudRenderLayer.SEARCH_PROGRESS, barX, barY, BAR_WIDTH, BAR_HEIGHT, TRACK_COLOR));
                int fill = Math.max(0, Math.round(state.progressRatio() * BAR_WIDTH));
                if (fill > 0) {
                    out.add(HudRenderCommand.rect(
                        HudRenderLayer.SEARCH_PROGRESS, barX, barY, fill, BAR_HEIGHT, FILL_COLOR));
                }
                String label = String.format(
                    "正在搜刮：%s [%ds]",
                    state.containerKindZh(),
                    state.remainingSeconds()
                );
                out.add(HudRenderCommand.text(
                    HudRenderLayer.SEARCH_PROGRESS, label, barX, barY - TEXT_OFFSET_Y, TEXT_COLOR));
            }
            case COMPLETED_FLASH -> {
                out.add(HudRenderCommand.rect(
                    HudRenderLayer.SEARCH_PROGRESS, barX, barY, BAR_WIDTH, BAR_HEIGHT, COMPLETED_COLOR));
                String label = String.format("搜刮完成：%s", state.containerKindZh());
                out.add(HudRenderCommand.text(
                    HudRenderLayer.SEARCH_PROGRESS, label, barX, barY - TEXT_OFFSET_Y, COMPLETED_COLOR));
            }
            case ABORTED_FLASH -> {
                String reasonZh = abortReasonLabel(state.abortReason());
                String label = String.format("搜刮中断：%s", reasonZh);
                out.add(HudRenderCommand.text(
                    HudRenderLayer.SEARCH_PROGRESS, label, barX, barY - TEXT_OFFSET_Y, ABORTED_COLOR));
            }
            case IDLE -> {
                // 已在上面 early-return。
            }
        }
        return out;
    }

    static String abortReasonLabel(SearchHudState.AbortReason reason) {
        return switch (reason) {
            case MOVED -> "位置偏移";
            case COMBAT -> "进入战斗";
            case DAMAGED -> "受击";
            case CANCELLED -> "已取消";
            case NONE -> "未知";
        };
    }
}
