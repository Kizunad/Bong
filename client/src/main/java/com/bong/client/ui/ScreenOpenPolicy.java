package com.bong.client.ui;

import java.util.Objects;

/**
 * R7 P4 的纯开屏仲裁策略。
 *
 * <p>策略不认识 Minecraft Screen，也不持有 pending offer。调用方负责把领域状态映射成
 * {@link Request} / {@link Current}，再执行返回的决策；因此普通热键不会被策略暗中排队。</p>
 */
public final class ScreenOpenPolicy {
    private ScreenOpenPolicy() {
    }

    public enum RequestKind {
        SOCIAL_INVITE,
        HOTKEY,
        INSIGHT,
        SYSTEM_TERMINAL
    }

    public enum CurrentKind {
        NONE,
        ORDINARY,
        MODAL,
        SYSTEM_TERMINAL
    }

    public enum TerminalPriority {
        NONE,
        DEATH,
        TERMINATE
    }

    public enum Decision {
        OPEN,
        PREEMPT,
        NOOP_MATCHING,
        DEFER_NOTIFY,
        DEFER_SILENT,
        BLOCK_DROP,
        EXPIRE
    }

    public record Request(
        RequestKind kind,
        String identity,
        long expiresAtMs,
        TerminalPriority terminalPriority,
        boolean alreadyNotified
    ) {
        public Request {
            Objects.requireNonNull(kind, "request kind must not be null");
            Objects.requireNonNull(terminalPriority, "request priority must not be null");
            identity = normalizeIdentity(identity);
            if (kind != RequestKind.SYSTEM_TERMINAL && terminalPriority != TerminalPriority.NONE) {
                throw new IllegalArgumentException(
                    "only system-terminal requests may carry terminal priority"
                );
            }
        }
    }

    public record Current(
        CurrentKind kind,
        String identity,
        TerminalPriority terminalPriority,
        boolean combatActive
    ) {
        public Current {
            Objects.requireNonNull(kind, "current kind must not be null");
            Objects.requireNonNull(terminalPriority, "current priority must not be null");
            identity = normalizeIdentity(identity);
            if (kind == CurrentKind.NONE && (!identity.isEmpty() || terminalPriority != TerminalPriority.NONE)) {
                throw new IllegalArgumentException("NONE current state must have empty identity and NONE priority");
            }
            if (kind != CurrentKind.SYSTEM_TERMINAL && terminalPriority != TerminalPriority.NONE) {
                throw new IllegalArgumentException(
                    "only system-terminal current state may carry terminal priority"
                );
            }
        }
    }

    /**
     * 按冻结顺序仲裁：过期先于 identity matching，随后按请求类别处理屏幕优先级。
     */
    public static Decision decide(Request request, Current current, long nowMs) {
        Objects.requireNonNull(request, "request must not be null");
        Objects.requireNonNull(current, "current must not be null");

        if (nowMs >= request.expiresAtMs()) {
            return Decision.EXPIRE;
        }
        if (matching(request, current)) {
            return Decision.NOOP_MATCHING;
        }

        return switch (request.kind()) {
            case SOCIAL_INVITE -> decideSocialInvite(request, current);
            case HOTKEY -> current.kind() == CurrentKind.NONE
                ? Decision.OPEN
                : Decision.BLOCK_DROP;
            case INSIGHT -> decideInsight(request, current);
            case SYSTEM_TERMINAL -> decideSystemTerminal(request, current);
        };
    }

    private static Decision decideSocialInvite(Request request, Current current) {
        if (current.kind() == CurrentKind.NONE && !current.combatActive()) {
            return Decision.OPEN;
        }
        return request.alreadyNotified() ? Decision.DEFER_SILENT : Decision.DEFER_NOTIFY;
    }

    private static Decision decideInsight(Request request, Current current) {
        return switch (current.kind()) {
            case NONE, ORDINARY -> current.kind() == CurrentKind.NONE
                ? Decision.OPEN
                : Decision.PREEMPT;
            case MODAL, SYSTEM_TERMINAL -> request.alreadyNotified()
                ? Decision.DEFER_SILENT
                : Decision.DEFER_NOTIFY;
        };
    }

    private static Decision decideSystemTerminal(Request request, Current current) {
        return switch (current.kind()) {
            case NONE -> Decision.OPEN;
            case ORDINARY, MODAL -> Decision.PREEMPT;
            case SYSTEM_TERMINAL -> comparePriority(
                request.terminalPriority(), current.terminalPriority()
            ) > 0 ? Decision.PREEMPT : Decision.BLOCK_DROP;
        };
    }

    private static boolean matching(Request request, Current current) {
        return !request.identity().isEmpty()
            && request.identity().equals(current.identity());
    }

    /** 显式优先级比较，避免未来调整 enum 声明顺序后改变终端行为。 */
    private static int comparePriority(TerminalPriority left, TerminalPriority right) {
        return Integer.compare(priorityValue(left), priorityValue(right));
    }

    private static int priorityValue(TerminalPriority priority) {
        return switch (priority) {
            case NONE -> 0;
            case DEATH -> 1;
            case TERMINATE -> 2;
        };
    }

    private static String normalizeIdentity(String identity) {
        return identity == null ? "" : identity.trim();
    }
}
