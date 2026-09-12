package com.bong.client.inspect;

/** 区分查看详情与拖动物品；点击本身不产生库存副作用。 */
public final class ItemInspectClickTracker {
    private static final long DOUBLE_CLICK_MS = 400;
    private static final double DRAG_DISTANCE_SQUARED = 16;

    public record Press(long instanceId, double x, double y) {}

    private Press pending;
    private Press previous;
    private long releasedAt;

    public boolean press(long instanceId, double x, double y, long now) {
        Press next = new Press(instanceId, x, y);
        boolean doubleClick = previous != null && previous.instanceId() == instanceId
            && now >= releasedAt && now - releasedAt <= DOUBLE_CLICK_MS
            && distanceSquared(previous, x, y) < DRAG_DISTANCE_SQUARED;
        previous = null;
        pending = doubleClick ? null : next;
        return doubleClick;
    }

    public Press drag(double x, double y) {
        if (pending == null || distanceSquared(pending, x, y) < DRAG_DISTANCE_SQUARED) return null;
        Press drag = pending;
        cancel();
        return drag;
    }

    public boolean release(double x, double y, long now) {
        if (pending == null) return false;
        previous = distanceSquared(pending, x, y) < DRAG_DISTANCE_SQUARED ? pending : null;
        releasedAt = now;
        pending = null;
        return true;
    }

    public void cancel() {
        pending = null;
        previous = null;
    }

    private static double distanceSquared(Press press, double x, double y) {
        double dx = x - press.x();
        double dy = y - press.y();
        return dx * dx + dy * dy;
    }
}
