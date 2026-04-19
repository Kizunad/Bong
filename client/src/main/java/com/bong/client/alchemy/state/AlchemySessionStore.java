package com.bong.client.alchemy.state;

import java.util.List;

// plan-alchemy-v1 P6 — 炼丹会话快照本地 Store。
public final class AlchemySessionStore {
    public record StageHint(int atTick, int window, String summary, boolean completed, boolean missed) {}

    public record Snapshot(
        String recipeId,
        int elapsedTicks,
        int targetTicks,
        float tempCurrent,
        float tempTarget,
        float tempBand,
        double qiInjected,
        double qiTarget,
        String statusLabel,
        List<StageHint> stages,
        List<String> interventionLog
    ) {
        public static Snapshot empty() {
            return new Snapshot(
                "kaimai_pill", 120, 200, 0.58f, 0.60f, 0.10f, 9.2, 15.0,
                "容差带内",
                List.of(),
                List.of(
                    "§7[t+58]  InjectQi(2.5)",
                    "§7[t+94]  AdjustTemp(-0.08)",
                    "§d[t+118] InjectQi(1.0) \u2190 刚刚"
                )
            );
        }

        public boolean isActive() {
            return recipeId != null && !recipeId.isEmpty();
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();

    private AlchemySessionStore() {
    }

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        snapshot = next == null ? Snapshot.empty() : next;
    }

    public static void resetForTests() {
        snapshot = Snapshot.empty();
    }
}
