package com.bong.client.combat;

import java.util.Objects;

/**
 * Single entry in the right-side unified event stream (§2.3 / §6). Immutable
 * aside from the fold-count, which is mutated in-place in
 * {@link UnifiedEventStream} for the &ldquo;×N&rdquo; overlay.
 */
public final class UnifiedEvent {

    public enum Channel {
        COMBAT("\u2694", 0xFFFF6060),        // crossed swords
        CULTIVATION("\u2728", 0xFF80FF80),   // sparkles
        WORLD("*", 0xFFFFFF80),              // world glyph (ascii fallback)
        SOCIAL(">", 0xFFA0C0FF),             // chat marker
        SYSTEM("+", 0xFF80FF80);             // system marker

        private final String icon;
        private final int defaultColor;

        Channel(String icon, int defaultColor) {
            this.icon = icon;
            this.defaultColor = defaultColor;
        }

        public String icon() { return icon; }
        public int defaultColor() { return defaultColor; }
    }

    public enum Priority {
        P0_CRITICAL(Long.MAX_VALUE),
        P1_IMPORTANT(6_000L),
        P2_NORMAL(4_000L),
        P3_VERBOSE(2_000L);

        private final long lifetimeMs;

        Priority(long lifetimeMs) {
            this.lifetimeMs = lifetimeMs;
        }

        public long lifetimeMs() { return lifetimeMs; }
    }

    private final Channel channel;
    private final Priority priority;
    private final String sourceTag;
    private final String text;
    private final int color;
    private final long createdAtMs;
    private int foldCount; // mutable by UnifiedEventStream only
    private long lastUpdatedMs;

    UnifiedEvent(
        Channel channel,
        Priority priority,
        String sourceTag,
        String text,
        int color,
        long createdAtMs
    ) {
        this.channel = Objects.requireNonNull(channel, "channel");
        this.priority = Objects.requireNonNull(priority, "priority");
        this.sourceTag = sourceTag == null ? "" : sourceTag;
        this.text = text == null ? "" : text;
        this.color = color;
        this.createdAtMs = createdAtMs;
        this.foldCount = 1;
        this.lastUpdatedMs = createdAtMs;
    }

    public Channel channel() { return channel; }
    public Priority priority() { return priority; }
    public String sourceTag() { return sourceTag; }
    public String text() { return text; }
    public int color() { return color; }
    public long createdAtMs() { return createdAtMs; }
    public int foldCount() { return foldCount; }
    public long lastUpdatedMs() { return lastUpdatedMs; }

    public String displayText() {
        return foldCount > 1
            ? (text + (foldCount >= 100 ? " \u00D799+" : " \u00D7" + foldCount))
            : text;
    }

    void bumpFold(long nowMs) {
        if (foldCount < 99) foldCount += 1;
        else foldCount = 100;
        lastUpdatedMs = nowMs;
    }

    /** Key used to identify &quot;same kind&quot; events within the fold window. */
    String foldKey() {
        return channel.name() + '|' + sourceTag + '|' + text;
    }
}
