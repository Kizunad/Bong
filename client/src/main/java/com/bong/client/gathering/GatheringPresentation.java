package com.bong.client.gathering;

/** 只平滑已确认的进度；终态保留首帧时间与中断前的弧段，不推算采集结果。 */
public record GatheringPresentation(GatheringSessionViewModel session, double fromProgress) {
    private static final long BLEND_MS = 150L;
    public static final long EXIT_MS = 1000L;

    public static GatheringPresentation of(GatheringSessionViewModel session) {
        return new GatheringPresentation(session, session.progressRatio());
    }

    public GatheringPresentation update(GatheringSessionViewModel next) {
        if (next.isEmpty() || !next.sessionId().equals(session.sessionId())) {
            return of(next);
        }
        if (session.completed() || session.interrupted()) {
            // 伐木会复用玩家 ID 作为 session ID，新的活动包代表下一次采集。
            if (next.active()) return of(next);
            GatheringSessionViewModel terminal = GatheringSessionViewModel.create(
                next.sessionId(), next.progressTicks(), next.totalTicks(), next.targetName(), next.targetType(),
                next.qualityHint(), next.toolUsed(), next.interrupted(), next.completed(), session.updatedAtMillis());
            return new GatheringPresentation(terminal, fromProgress);
        }
        return new GatheringPresentation(next, progress(next.updatedAtMillis()));
    }

    public double progress(long nowMs) {
        if (session.interrupted()) return fromProgress;
        if (session.completed()) return 1.0;
        double t = Math.max(0.0, Math.min(1.0, (double) age(nowMs) / BLEND_MS));
        double eased = t * t * (3.0 - 2.0 * t);
        return fromProgress + (session.progressRatio() - fromProgress) * eased;
    }

    public long age(long nowMs) {
        return Math.max(0L, nowMs - session.updatedAtMillis());
    }

    public boolean visible(long nowMs) {
        return !session.isEmpty() && (session.active() || age(nowMs) < EXIT_MS);
    }
}
