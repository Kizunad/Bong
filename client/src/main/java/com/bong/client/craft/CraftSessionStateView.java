package com.bong.client.craft;

import java.util.Objects;
import java.util.Optional;

/**
 * plan-craft-v1 §3 — 当前 craft session 进度的客户端视图。
 *
 * <p>对应 server `CraftSessionStateV1`。`active=false` 时 `recipeId` 为空，
 * UI 应隐藏进度条；`active=true` 时按 `elapsedTicks/totalTicks` 渲染。</p>
 */
public final class CraftSessionStateView {
    public static final CraftSessionStateView IDLE =
        new CraftSessionStateView(false, null, 0L, 0L);

    private final boolean active;
    private final String recipeId;
    private final long elapsedTicks;
    private final long totalTicks;

    public CraftSessionStateView(boolean active, String recipeId, long elapsedTicks, long totalTicks) {
        this.active = active;
        this.recipeId = recipeId;
        this.elapsedTicks = elapsedTicks;
        this.totalTicks = totalTicks;
    }

    public boolean active() { return active; }
    public Optional<String> recipeId() { return Optional.ofNullable(recipeId); }
    public long elapsedTicks() { return elapsedTicks; }
    public long totalTicks() { return totalTicks; }

    /** 0..1 进度比例。`totalTicks=0` 视为 0。 */
    public float progress() {
        if (totalTicks <= 0) return 0f;
        float ratio = (float) elapsedTicks / (float) totalTicks;
        if (ratio < 0f) return 0f;
        if (ratio > 1f) return 1f;
        return ratio;
    }

    /** 剩余 in-game 秒数（向上取整，按 20 tick/s）。 */
    public long remainingSeconds() {
        long remaining = Math.max(0L, totalTicks - elapsedTicks);
        return (remaining + 19L) / 20L;
    }

    @Override
    public boolean equals(Object obj) {
        if (this == obj) return true;
        if (!(obj instanceof CraftSessionStateView other)) return false;
        return active == other.active
            && elapsedTicks == other.elapsedTicks
            && totalTicks == other.totalTicks
            && Objects.equals(recipeId, other.recipeId);
    }

    @Override
    public int hashCode() {
        return Objects.hash(active, recipeId, elapsedTicks, totalTicks);
    }
}
