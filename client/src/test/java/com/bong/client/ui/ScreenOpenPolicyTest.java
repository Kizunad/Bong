package com.bong.client.ui;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

class ScreenOpenPolicyTest {
    private static final long NOW = 1_000L;
    private static final long LIVE = 1_001L;
    private static final long NEVER = Long.MAX_VALUE;

    @Test
    void expiryWinsBeforeMatchingAtBoundary() {
        ScreenOpenPolicy.Request request = request(
            ScreenOpenPolicy.RequestKind.SOCIAL_INVITE, "invite", 1_000L, false
        );
        ScreenOpenPolicy.Current current = current(
            ScreenOpenPolicy.CurrentKind.MODAL, "invite", ScreenOpenPolicy.TerminalPriority.NONE, false
        );

        assertEquals(ScreenOpenPolicy.Decision.EXPIRE,
            ScreenOpenPolicy.decide(request, current, NOW),
            "nowMs 等于 expiresAtMs 时必须先过期，不能被 matching 短路");
    }

    @Test
    void socialInviteDefersOnceAndThenSilently() {
        ScreenOpenPolicy.Current blocked = current(
            ScreenOpenPolicy.CurrentKind.ORDINARY, "inventory", ScreenOpenPolicy.TerminalPriority.NONE, false
        );

        assertEquals(ScreenOpenPolicy.Decision.DEFER_NOTIFY,
            ScreenOpenPolicy.decide(request(ScreenOpenPolicy.RequestKind.SOCIAL_INVITE, "invite", LIVE, false), blocked, NOW));
        assertEquals(ScreenOpenPolicy.Decision.DEFER_SILENT,
            ScreenOpenPolicy.decide(request(ScreenOpenPolicy.RequestKind.SOCIAL_INVITE, "invite", LIVE, true), blocked, NOW));
        assertEquals(ScreenOpenPolicy.Decision.OPEN,
            ScreenOpenPolicy.decide(request(ScreenOpenPolicy.RequestKind.SOCIAL_INVITE, "invite", LIVE, false),
                current(ScreenOpenPolicy.CurrentKind.NONE, "", ScreenOpenPolicy.TerminalPriority.NONE, false), NOW));
    }

    @Test
    void hotkeyDropsBehindAnyScreenAndNeverQueues() {
        ScreenOpenPolicy.Request request = request(
            ScreenOpenPolicy.RequestKind.HOTKEY, "identity", NEVER, false
        );

        assertEquals(ScreenOpenPolicy.Decision.OPEN,
            ScreenOpenPolicy.decide(request,
                current(ScreenOpenPolicy.CurrentKind.NONE, "", ScreenOpenPolicy.TerminalPriority.NONE, true), NOW));
        assertEquals(ScreenOpenPolicy.Decision.BLOCK_DROP,
            ScreenOpenPolicy.decide(request,
                current(ScreenOpenPolicy.CurrentKind.MODAL, "trade", ScreenOpenPolicy.TerminalPriority.NONE, false), NOW));
    }

    @Test
    void insightPreemptsOrdinaryButDefersBehindModal() {
        ScreenOpenPolicy.Request request = request(
            ScreenOpenPolicy.RequestKind.INSIGHT, "insight", LIVE, false
        );

        assertEquals(ScreenOpenPolicy.Decision.PREEMPT,
            ScreenOpenPolicy.decide(request,
                current(ScreenOpenPolicy.CurrentKind.ORDINARY, "inventory", ScreenOpenPolicy.TerminalPriority.NONE, false), NOW));
        assertEquals(ScreenOpenPolicy.Decision.DEFER_NOTIFY,
            ScreenOpenPolicy.decide(request,
                current(ScreenOpenPolicy.CurrentKind.MODAL, "trade", ScreenOpenPolicy.TerminalPriority.NONE, false), NOW));
    }

    @Test
    void terminateExplicitlyOutranksDeath() {
        ScreenOpenPolicy.Current death = current(
            ScreenOpenPolicy.CurrentKind.SYSTEM_TERMINAL, "death", ScreenOpenPolicy.TerminalPriority.DEATH, false
        );
        ScreenOpenPolicy.Current terminate = current(
            ScreenOpenPolicy.CurrentKind.SYSTEM_TERMINAL, "terminate", ScreenOpenPolicy.TerminalPriority.TERMINATE, false
        );

        assertEquals(ScreenOpenPolicy.Decision.PREEMPT,
            ScreenOpenPolicy.decide(request(ScreenOpenPolicy.RequestKind.SYSTEM_TERMINAL, "terminate-2", NEVER,
                    ScreenOpenPolicy.TerminalPriority.TERMINATE), death, NOW));
        assertEquals(ScreenOpenPolicy.Decision.BLOCK_DROP,
            ScreenOpenPolicy.decide(request(ScreenOpenPolicy.RequestKind.SYSTEM_TERMINAL, "death-2", NEVER,
                    ScreenOpenPolicy.TerminalPriority.DEATH), terminate, NOW));
    }

    @Test
    void matchingIdentityWinsAfterLiveExpiryCheck() {
        ScreenOpenPolicy.Request request = request(
            ScreenOpenPolicy.RequestKind.INSIGHT, " insight ", LIVE, false
        );
        ScreenOpenPolicy.Current current = current(
            ScreenOpenPolicy.CurrentKind.MODAL, "insight", ScreenOpenPolicy.TerminalPriority.NONE, false
        );

        assertEquals(ScreenOpenPolicy.Decision.NOOP_MATCHING,
            ScreenOpenPolicy.decide(request, current, NOW));
    }

    @Test
    void invalidTerminalPriorityCombinationsAreRejected() {
        assertThrows(IllegalArgumentException.class,
            () -> request(ScreenOpenPolicy.RequestKind.HOTKEY, "hotkey", NEVER,
                ScreenOpenPolicy.TerminalPriority.DEATH));
        assertThrows(IllegalArgumentException.class,
            () -> current(ScreenOpenPolicy.CurrentKind.ORDINARY, "ordinary",
                ScreenOpenPolicy.TerminalPriority.DEATH, false));
    }

    @Test
    void everyFrozenTsvVectorMatchesTheProductionPolicy() throws IOException {
        List<String> lines;
        try (var stream = ScreenOpenPolicyTest.class
            .getResourceAsStream("/bong/ui/screen-open-policy.tsv")) {
            if (stream == null) {
                throw new AssertionError("缺少 ScreenOpenPolicy 冻结向量");
            }
            lines = new String(stream.readAllBytes(), StandardCharsets.UTF_8)
                .lines()
                .filter(line -> !line.isBlank() && !line.startsWith("#"))
                .toList();
        }

        assertEquals(35, lines.size(), "生产策略必须覆盖全部 35 条冻结向量");
        for (String line : lines) {
            String[] columns = line.split("\\t", -1);
            assertEquals(13, columns.length, "策略向量列数损坏: " + line);
            ScreenOpenPolicy.Request request = new ScreenOpenPolicy.Request(
                ScreenOpenPolicy.RequestKind.valueOf(columns[1]),
                columns[2],
                Long.parseLong(columns[3]),
                ScreenOpenPolicy.TerminalPriority.valueOf(columns[4]),
                Boolean.parseBoolean(columns[5])
            );
            ScreenOpenPolicy.Current current = new ScreenOpenPolicy.Current(
                ScreenOpenPolicy.CurrentKind.valueOf(columns[6]),
                columns[7],
                ScreenOpenPolicy.TerminalPriority.valueOf(columns[8]),
                Boolean.parseBoolean(columns[9])
            );
            ScreenOpenPolicy.Decision actual = ScreenOpenPolicy.decide(
                request, current, Long.parseLong(columns[10])
            );
            assertEquals(ScreenOpenPolicy.Decision.valueOf(columns[11]), actual,
                "策略向量不一致: " + columns[0] + "，原因: " + columns[12]);
        }
    }

    private static ScreenOpenPolicy.Request request(
        ScreenOpenPolicy.RequestKind kind,
        String identity,
        long expiresAtMs,
        boolean alreadyNotified
    ) {
        return request(kind, identity, expiresAtMs, ScreenOpenPolicy.TerminalPriority.NONE, alreadyNotified);
    }

    private static ScreenOpenPolicy.Request request(
        ScreenOpenPolicy.RequestKind kind,
        String identity,
        long expiresAtMs,
        ScreenOpenPolicy.TerminalPriority priority
    ) {
        return request(kind, identity, expiresAtMs, priority, false);
    }

    private static ScreenOpenPolicy.Request request(
        ScreenOpenPolicy.RequestKind kind,
        String identity,
        long expiresAtMs,
        ScreenOpenPolicy.TerminalPriority priority,
        boolean alreadyNotified
    ) {
        return new ScreenOpenPolicy.Request(kind, identity, expiresAtMs, priority, alreadyNotified);
    }

    private static ScreenOpenPolicy.Current current(
        ScreenOpenPolicy.CurrentKind kind,
        String identity,
        ScreenOpenPolicy.TerminalPriority priority,
        boolean combatActive
    ) {
        return new ScreenOpenPolicy.Current(kind, identity, priority, combatActive);
    }
}
