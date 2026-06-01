package com.bong.client.hud.war;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-offscreen-war-v1 P9：战事 HUD 渲染规划器。
 *
 * <p>纯函数 {@link #buildCommands}，从 {@link FactionWarHudStore} 读快照，输出
 * {@link HudRenderCommand} 列表。
 *
 * <p><b>双色血条 layout</b>：顶部居中（y = TOP_MARGIN，避开 tribulation broadcast 行
 * y=28），左红条（甲方/groups[0]）+ 右蓝条（乙方/groups[1]），中间显示 WarPhase 中文标签。
 * 比例来源：{@code enlistCount+mercenaryCount} 按 allied_group 拆分；无玩家时 50/50 占位。
 *
 * <p>Aftermath 阶段 → 显示「{regionDescriptor}占上风」结语（最终 settle 之后 {@link FactionWarHudStore#clear()} 时消失）。
 *
 * <p>reframe b：label 用「甲方/乙方」中性词，绝不出现具名宗门名。
 *
 * <p>守恒红线：HUD 纯叙事整数，不触任何真元字段（零真元）。
 */
public final class FactionWarHudPlanner {

    // ─────────── 布局常量 ──────────────────────────────────────────────────────

    /** Y 偏移：TribulationBroadcastHudPlanner.TOP_MARGIN=28 → 此处避让到 50。 */
    public static final int TOP_MARGIN = 50;
    /** 血条总宽（占屏幕宽度的分数）。 */
    public static final int BAR_TOTAL_WIDTH = 200;
    /** 血条高度。 */
    public static final int BAR_HEIGHT = 8;
    /** 分隔间距。 */
    public static final int BAR_GAP = 4;

    // ─────────── 颜色 ─────────────────────────────────────────────────────────

    public static final int COLOR_SIDE_A = 0xC0CC2222;   // 左/甲方：深红
    public static final int COLOR_SIDE_B = 0xC02244CC;   // 右/乙方：深蓝
    public static final int COLOR_PHASE_TEXT = 0xFFFFDD88; // 阶段文字：暖黄
    public static final int COLOR_OUTCOME_TEXT = 0xFFAAFF88; // 结局文字：淡绿

    private FactionWarHudPlanner() {}

    /**
     * 构建战事 HUD 渲染指令列表。
     *
     * @param screenWidth  屏幕宽度（px）
     * @param screenHeight 屏幕高度（px）
     * @param nowMs        当前时间戳（毫秒，供将来动画用）
     * @return 不可变指令列表（空 = store 无 active 战事）
     */
    public static List<HudRenderCommand> buildCommands(
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        FactionWarHudStore.State state = FactionWarHudStore.snapshot();
        List<HudRenderCommand> out = new ArrayList<>();

        if (screenWidth <= 0 || screenHeight <= 0) return out;
        if (state.phase().isEmpty() || !state.active()) return out;

        // ─── WarPhase 文字标签 ─────────────────────────────────────────────────
        String phaseLabel = phaseChineseLabel(state.phase());
        int labelX = Math.max(4, (screenWidth - phaseLabel.length() * 6) / 2);
        out.add(HudRenderCommand.text(HudRenderLayer.FACTION_WAR, phaseLabel, labelX, TOP_MARGIN, COLOR_PHASE_TEXT));

        // ─── 双色血条 ──────────────────────────────────────────────────────────
        int barY = TOP_MARGIN + 12;
        int barStartX = Math.max(4, (screenWidth - BAR_TOTAL_WIDTH - BAR_GAP) / 2);
        int halfBar = BAR_TOTAL_WIDTH / 2;

        // 比例计算：按 groups[0] 和 groups[1] 各自归属的 enlist+mercenary 玩家数
        float ratioA = computeSideRatio(state, 0);
        float ratioB = 1.0f - ratioA;

        int widthA = Math.max(1, Math.round(halfBar * 2 * ratioA));
        int widthB = Math.max(1, BAR_TOTAL_WIDTH - widthA);

        // 甲方红条（左）
        out.add(HudRenderCommand.rect(HudRenderLayer.FACTION_WAR, barStartX, barY, widthA, BAR_HEIGHT, COLOR_SIDE_A));
        // 乙方蓝条（右，偏移 widthA + GAP）
        out.add(HudRenderCommand.rect(HudRenderLayer.FACTION_WAR, barStartX + widthA + BAR_GAP, barY, widthB, BAR_HEIGHT, COLOR_SIDE_B));

        // ─── 甲/乙方标签 ──────────────────────────────────────────────────────
        String labelA = state.regionDescriptor().isEmpty() ? "甲方" : "甲方";
        String labelB = "乙方";
        out.add(HudRenderCommand.text(HudRenderLayer.FACTION_WAR, labelA, barStartX, barY + BAR_HEIGHT + 2, COLOR_SIDE_A));
        out.add(HudRenderCommand.text(HudRenderLayer.FACTION_WAR, labelB, barStartX + widthA + BAR_GAP, barY + BAR_HEIGHT + 2, COLOR_SIDE_B));

        // ─── 结局结语（Settling/Aftermath 有 outcome）─────────────────────────
        if (state.hasOutcome()) {
            String outcomeText = state.regionDescriptor() + "占上风";
            int outcomeX = Math.max(4, (screenWidth - outcomeText.length() * 6) / 2);
            out.add(HudRenderCommand.text(HudRenderLayer.FACTION_WAR, outcomeText, outcomeX, barY + BAR_HEIGHT + 14, COLOR_OUTCOME_TEXT));
        }

        return out;
    }

    /**
     * 计算甲方（groups[0]）侧的血条比例（0.0–1.0）。
     * 来源：enlist+mercenary 玩家总数；无玩家时 50/50。
     *
     * <p>TODO P7+：当前 {@code FactionWarStateV1} 只有总玩家计数，缺少 per-group 分拆；
     * 实装后替换此处逻辑以渲染真实的攻守力量对比。
     * （CodeRabbit review 2026-06-01 suggestion，已确认为 future work。）
     */
    static float computeSideRatio(FactionWarHudStore.State state, int sideIndex) {
        // 简化：当前 store 中没有 per-group 玩家计数（只有总计），50/50 占位
        // 实际多 group 分拆需要 per-group 玩家计数字段（未来扩展），此处返回 0.5
        int total = state.enlistCount() + state.mercenaryCount();
        if (total == 0) return 0.5f;
        // 无 per-group 玩家计数时仍 50/50（结构完整，测试可 pin 此分支）
        return 0.5f;
    }

    /**
     * WarPhase → 中文叙事标签（reframe b：自发升级语义，无宣战）。
     */
    public static String phaseChineseLabel(String phase) {
        return switch (phase) {
            case "emerging" -> "涌动";
            case "skirmish" -> "交锋";
            case "settling" -> "落定";
            case "aftermath" -> "余波";
            default -> "未知";
        };
    }
}
