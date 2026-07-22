package com.bong.client.alchemy;

import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemySessionStore;

import java.util.ArrayList;
import java.util.List;

/**
 * Headless presentation seam shared by {@link AlchemyScreen} and protobuf/store contract tests.
 * It renders the latest authoritative session snapshot for the furnace screen. Active guidance is
 * distinct from terminal guidance: a finished snapshot may retain targets and stages for review,
 * while an empty furnace still clears stale terminal data.
 */
public final class AlchemySessionPresentationPlanner {
    public record Presentation(
        boolean idle,
        boolean active,
        boolean terminal,
        String statusText,
        String progressText,
        String temperatureText,
        String qiText,
        List<String> detailLines,
        List<Integer> flashingStageSlots
    ) {
        public Presentation {
            detailLines = detailLines == null ? List.of() : List.copyOf(detailLines);
            flashingStageSlots = flashingStageSlots == null
                ? List.of()
                : List.copyOf(flashingStageSlots);
        }
    }

    private AlchemySessionPresentationPlanner() {
    }

    public static Presentation describe(
        AlchemyFurnaceStore.Snapshot furnace,
        AlchemySessionStore.Snapshot session
    ) {
        AlchemyFurnaceStore.Snapshot safeFurnace = furnace == null
            ? AlchemyFurnaceStore.Snapshot.empty()
            : furnace;
        AlchemySessionStore.Snapshot safeSession = session == null
            ? AlchemySessionStore.Snapshot.empty()
            : session;

        if (!safeFurnace.hasSession()) {
            return new Presentation(
                true,
                false,
                false,
                "§8未起炉",
                "§70 / 0t",
                "",
                "",
                List.of("§7干预"),
                List.of()
            );
        }

        if (safeSession.isActive()) {
            return new Presentation(
                false,
                true,
                false,
                String.format(
                    "§e%.2f / %.2f %s",
                    safeSession.tempCurrent(),
                    safeSession.tempTarget(),
                    statusOr(safeSession.statusLabel(), "炼制中")
                ),
                progressText(safeSession),
                temperatureText(safeSession),
                qiText(safeSession),
                interventionLines(safeSession),
                flashingStageSlots(safeSession)
            );
        }

        if (hasAuthoritativeGuidance(safeSession)) {
            return new Presentation(
                false,
                false,
                true,
                "§a已结束 · §f" + statusOr(safeSession.statusLabel(), "已结束"),
                progressText(safeSession),
                temperatureText(safeSession),
                qiText(safeSession),
                finishedGuidanceLines(safeSession),
                List.of()
            );
        }

        return new Presentation(
            false,
            false,
            false,
            "§c炉内会话数据缺失 · §f" + statusOr(safeSession.statusLabel(), "等待同步"),
            safeSession.targetTicks() > 0 ? progressText(safeSession) : "§7同步中",
            safeSession.tempTarget() > 0.0f ? temperatureText(safeSession) : "",
            safeSession.qiTarget() > 0.0 ? qiText(safeSession) : "",
            interventionLines(safeSession),
            List.of()
        );
    }

    private static boolean hasAuthoritativeGuidance(AlchemySessionStore.Snapshot session) {
        return session.recipeId() != null
            && !session.recipeId().isBlank()
            && session.targetTicks() > 0;
    }

    private static String progressText(AlchemySessionStore.Snapshot session) {
        return String.format("§f%d / %dt", session.elapsedTicks(), session.targetTicks());
    }

    private static String temperatureText(AlchemySessionStore.Snapshot session) {
        return String.format("§e%.2f / %.2f", session.tempCurrent(), session.tempTarget());
    }

    private static String qiText(AlchemySessionStore.Snapshot session) {
        return String.format("§7%.1f / %.1f", session.qiInjected(), session.qiTarget());
    }

    private static List<Integer> flashingStageSlots(AlchemySessionStore.Snapshot session) {
        List<Integer> slots = new ArrayList<>();
        int elapsed = session.elapsedTicks();
        for (int index = 0; index < session.stages().size(); index++) {
            AlchemySessionStore.StageHint stage = session.stages().get(index);
            if (stage.completed() || stage.missed()) continue;
            int end = stage.atTick() + stage.window();
            if (elapsed >= stage.atTick() && elapsed <= end) {
                slots.add(index);
            }
        }
        return slots;
    }

    private static List<String> interventionLines(AlchemySessionStore.Snapshot session) {
        List<String> lines = new ArrayList<>();
        lines.add("§7干预");
        session.interventionLog().stream().limit(2).forEach(lines::add);
        return lines;
    }

    private static List<String> finishedGuidanceLines(AlchemySessionStore.Snapshot session) {
        List<String> lines = new ArrayList<>();
        lines.add("§7阶段 / 干预");
        for (AlchemySessionStore.StageHint stage : session.stages()) {
            String state = stage.completed() ? "§a✓" : stage.missed() ? "§c×" : "§e○";
            String summary = stage.summary() == null || stage.summary().isBlank()
                ? "（无投料）"
                : stage.summary();
            lines.add(String.format(
                "%s §7t%d (+%d) §f%s",
                state,
                stage.atTick(),
                stage.window(),
                summary
            ));
        }
        session.interventionLog().stream()
            .limit(2)
            .map(line -> "§7干预：" + line)
            .forEach(lines::add);
        return lines;
    }

    private static String statusOr(String status, String fallback) {
        return status == null || status.isBlank() ? fallback : status;
    }
}
