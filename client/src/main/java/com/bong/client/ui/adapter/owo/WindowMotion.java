package com.bong.client.ui.adapter.owo;

import com.bong.client.ui.window.UiWindowManager.Rect;

/** 新目标从当前插值矩形继续，拖动和降低动态效果时直接跟随目标。 */
public final class WindowMotion {
    private Rect from;
    private Rect target;
    private long started;
    private long duration;

    public WindowMotion(Rect initial) { from = target = initial; }

    public void target(Rect next, long now, boolean animate) {
        if (next.equals(target) && animate) return;
        from = sample(now);
        target = next;
        started = now;
        duration = animate ? 180_000_000L : 0;
    }

    public Rect sample(long now) {
        double time = duration == 0 ? 1 : Math.max(0, Math.min((double) (now - started) / duration, 1));
        double t = 1 - Math.pow(1 - time, 3);
        return new Rect(mix(from.x(), target.x(), t), mix(from.y(), target.y(), t),
            Math.max(1, mix(from.width(), target.width(), t)), Math.max(1, mix(from.height(), target.height(), t)));
    }

    public boolean settled(long now) { return duration == 0 || now - started >= duration; }
    private static int mix(int a, int b, double t) { return (int) Math.round(a + ((double) b - a) * t); }
}
