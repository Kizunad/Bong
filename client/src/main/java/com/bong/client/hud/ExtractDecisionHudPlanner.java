package com.bong.client.hud;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.state.SeasonState;

import java.util.ArrayList;
import java.util.List;

public final class ExtractDecisionHudPlanner {
    static final long RETURN_HINT_MS = 40L * 60L * 1000L;
    static final double QI_DANGER_RATIO = 0.30;
    static final double QI_WATCH_RATIO = 0.50;
    static final double BAG_WARN_RATIO = 0.80;
    static final double BAG_WATCH_RATIO = 0.65;
    static final int RISK_WARN_SCORE = 60;
    static final int RISK_CRITICAL_SCORE = 80;

    private static final int PANEL_WIDTH = 142;
    private static final int PANEL_HEIGHT = 88;
    private static final int TRACK_WIDTH = 44;
    private static final int TRACK_HEIGHT = 3;
    private static final int PANEL_BG = 0xA0101512;
    private static final int BORDER = 0x805F745F;
    private static final int TRACK_BG = 0x70202A24;
    private static final int TEXT = 0xFFE9F2DD;
    private static final int MUTED = 0xFF9EAD99;
    private static final int EMERGENCY = 0xE0D04030;

    private ExtractDecisionHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        PlayerStateViewModel playerState,
        InventoryModel inventory,
        SeasonState season,
        ExtractDecisionStateStore.Snapshot decisionState,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        PlayerStateViewModel safePlayer = playerState == null ? PlayerStateViewModel.empty() : playerState;
        InventoryModel safeInventory = inventory == null ? InventoryModel.empty() : inventory;
        SeasonState safeSeason = season == null ? SeasonState.summerAt(0L) : season;
        ExtractDecisionStateStore.Snapshot safeDecision =
            decisionState == null ? ExtractDecisionStateStore.Snapshot.EMPTY : decisionState;

        if (screenWidth <= 0 || screenHeight <= 0 || widthMeasurer == null) {
            return List.of();
        }
        if (safePlayer.isEmpty() && !safeDecision.active(nowMillis)) {
            return List.of();
        }

        List<DecisionRow> rows = buildRows(safePlayer, safeInventory, safeSeason, safeDecision, nowMillis);
        int x = Math.max(8, screenWidth - PANEL_WIDTH - 12);
        int y = Math.max(34, (screenHeight - PANEL_HEIGHT) / 2);
        List<HudRenderCommand> commands = new ArrayList<>();

        if (safeDecision.emergencyActive(nowMillis)) {
            commands.add(HudRenderCommand.edgeVignette(HudRenderLayer.EXTRACT_DECISION, 0xA0D04030));
            commands.add(HudRenderCommand.text(
                HudRenderLayer.EXTRACT_DECISION,
                "快撤",
                Math.max(8, screenWidth / 2 - 14),
                Math.max(18, screenHeight / 3),
                0xFFFFC6A8
            ));
        }

        appendPanel(commands, x, y, PANEL_WIDTH, PANEL_HEIGHT);
        String title = rows.stream().anyMatch(row -> row.severity().atLeast(Severity.WARNING))
            ? "该看看这些了"
            : "出行脉象";
        commands.add(text(widthMeasurer, title, x + 8, y + 6, PANEL_WIDTH - 16, TEXT));

        int rowY = y + 20;
        for (DecisionRow row : rows) {
            appendRow(commands, widthMeasurer, row, x + 8, rowY);
            rowY += 12;
        }
        return List.copyOf(commands);
    }

    static List<DecisionRow> buildRows(
        PlayerStateViewModel playerState,
        InventoryModel inventory,
        SeasonState season,
        ExtractDecisionStateStore.Snapshot decisionState,
        long nowMillis
    ) {
        List<DecisionRow> rows = new ArrayList<>();
        rows.add(qiRow(playerState, inventory));
        rows.add(bagRow(inventory));
        rows.add(timeRow(decisionState, nowMillis));
        rows.add(tideRow(season));
        rows.add(threatRow(decisionState));
        return List.copyOf(rows);
    }

    private static DecisionRow qiRow(PlayerStateViewModel playerState, InventoryModel inventory) {
        if ((playerState == null || playerState.isEmpty()) && (inventory == null || inventory.isEmpty())) {
            return new DecisionRow("真元", "未明", Severity.STEADY, 0.0);
        }
        double ratio = playerState == null || playerState.isEmpty()
            ? inventory.qiFillRatio()
            : playerState.spiritQiFillRatio();
        Severity severity = ratio < QI_DANGER_RATIO
            ? Severity.CRITICAL
            : ratio < QI_WATCH_RATIO ? Severity.WARNING : Severity.STEADY;
        String status = severity == Severity.CRITICAL ? "虚" : severity == Severity.WARNING ? "紧" : "稳";
        return new DecisionRow("真元", status, severity, ratio);
    }

    private static DecisionRow bagRow(InventoryModel inventory) {
        double ratio = inventory.maxWeight() <= 0.0 ? 0.0 : inventory.currentWeight() / inventory.maxWeight();
        Severity severity = ratio > BAG_WARN_RATIO
            ? Severity.WARNING
            : ratio > BAG_WATCH_RATIO ? Severity.WATCH : Severity.STEADY;
        String status = severity == Severity.WARNING ? "胀" : severity == Severity.WATCH ? "沉" : "松";
        return new DecisionRow("背包", status, severity, ratio);
    }

    private static DecisionRow timeRow(ExtractDecisionStateStore.Snapshot decisionState, long nowMillis) {
        if (decisionState == null || !decisionState.hasRunClock()) {
            return new DecisionRow("时辰", "在途", Severity.STEADY, 0.0);
        }
        long elapsed = Math.max(0L, nowMillis - decisionState.runStartedAtMs());
        double ratio = elapsed / (double) RETURN_HINT_MS;
        Severity severity = ratio >= 1.0 ? Severity.WARNING : ratio >= 0.75 ? Severity.WATCH : Severity.STEADY;
        String status = severity == Severity.WARNING ? "该回" : severity == Severity.WATCH ? "久" : "早";
        return new DecisionRow("时辰", status, severity, ratio);
    }

    private static DecisionRow tideRow(SeasonState season) {
        boolean tideTurn = season != null && season.phase().tideTurn();
        return new DecisionRow("汐转", tideTurn ? "不稳" : "稳", tideTurn ? Severity.WARNING : Severity.STEADY, tideTurn ? 1.0 : 0.0);
    }

    private static DecisionRow threatRow(ExtractDecisionStateStore.Snapshot decisionState) {
        if (decisionState == null || !decisionState.hasRiskScore()) {
            return new DecisionRow("威胁", "静", Severity.STEADY, 0.0);
        }
        int riskScore = decisionState.nearbyRiskScore();
        Severity severity = riskScore >= RISK_CRITICAL_SCORE
            ? Severity.CRITICAL
            : riskScore > RISK_WARN_SCORE ? Severity.WARNING : riskScore >= 40 ? Severity.WATCH : Severity.STEADY;
        String status = switch (severity) {
            case CRITICAL -> "逼近";
            case WARNING -> "危险";
            case WATCH -> "有声";
            case STEADY -> "静";
        };
        return new DecisionRow("威胁", status, severity, riskScore / 100.0);
    }

    private static void appendPanel(List<HudRenderCommand> commands, int x, int y, int width, int height) {
        commands.add(HudRenderCommand.rect(HudRenderLayer.EXTRACT_DECISION, x, y, width, height, PANEL_BG));
        commands.add(HudRenderCommand.rect(HudRenderLayer.EXTRACT_DECISION, x, y, width, 1, BORDER));
        commands.add(HudRenderCommand.rect(HudRenderLayer.EXTRACT_DECISION, x, y + height - 1, width, 1, BORDER));
        commands.add(HudRenderCommand.rect(HudRenderLayer.EXTRACT_DECISION, x, y, 1, height, BORDER));
        commands.add(HudRenderCommand.rect(HudRenderLayer.EXTRACT_DECISION, x + width - 1, y, 1, height, BORDER));
    }

    private static void appendRow(
        List<HudRenderCommand> commands,
        HudTextHelper.WidthMeasurer widthMeasurer,
        DecisionRow row,
        int x,
        int y
    ) {
        int color = row.severity().color();
        commands.add(text(widthMeasurer, row.label(), x, y, 28, color));
        commands.add(HudRenderCommand.rect(HudRenderLayer.EXTRACT_DECISION, x + 34, y + 4, TRACK_WIDTH, TRACK_HEIGHT, TRACK_BG));
        int fill = Math.max(2, Math.min(TRACK_WIDTH, (int) Math.round(TRACK_WIDTH * clamp01(row.fillRatio()))));
        commands.add(HudRenderCommand.rect(HudRenderLayer.EXTRACT_DECISION, x + 34, y + 4, fill, TRACK_HEIGHT, color));
        commands.add(text(widthMeasurer, row.status(), x + 86, y, 38, color));
    }

    private static HudRenderCommand text(
        HudTextHelper.WidthMeasurer widthMeasurer,
        String text,
        int x,
        int y,
        int maxWidth,
        int color
    ) {
        return HudRenderCommand.text(
            HudRenderLayer.EXTRACT_DECISION,
            HudTextHelper.clipToWidth(text, maxWidth, widthMeasurer),
            x,
            y,
            color
        );
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }

    enum Severity {
        STEADY(0xFF8FD58A),
        WATCH(0xFFD8C36A),
        WARNING(0xFFE0964D),
        CRITICAL(EMERGENCY);

        private final int color;

        Severity(int color) {
            this.color = color;
        }

        int color() {
            return color;
        }

        boolean atLeast(Severity other) {
            return ordinal() >= other.ordinal();
        }
    }

    record DecisionRow(String label, String status, Severity severity, double fillRatio) {
        DecisionRow {
            label = label == null ? "" : label;
            status = status == null ? "" : status;
            severity = severity == null ? Severity.STEADY : severity;
            fillRatio = clamp01(fillRatio);
        }
    }
}
