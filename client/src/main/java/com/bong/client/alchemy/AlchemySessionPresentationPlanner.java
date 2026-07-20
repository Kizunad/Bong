package com.bong.client.alchemy;

import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemySessionStore;

import java.util.ArrayList;
import java.util.List;

/**
 * Headless presentation seam shared by {@link AlchemyScreen} and protobuf/store contract tests.
 * It keeps the furnace-presence bit authoritative: a retained inactive session is waiting for
 * take-back, while an empty furnace clears even if the terminal session packet still carries
 * completed guidance for diagnostics.
 */
public final class AlchemySessionPresentationPlanner {
    public record Presentation(
        boolean idle,
        boolean active,
        boolean finishedUnclaimed,
        String statusText,
        String progressText,
        String temperatureText,
        String qiText,
        List<String> detailLines
    ) {
        public Presentation {
            detailLines = detailLines == null ? List.of() : List.copyOf(detailLines);
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
                List.of("§7干预")
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
                interventionLines(safeSession)
            );
        }

        if (hasAuthoritativeGuidance(safeSession)) {
            return new Presentation(
                false,
                false,
                true,
                "§a已完成 · 等待按 T 取回 · §f"
                    + statusOr(safeSession.statusLabel(), "已结束"),
                progressText(safeSession),
                temperatureText(safeSession),
                qiText(safeSession),
                finishedGuidanceLines(safeSession)
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
            interventionLines(safeSession)
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
