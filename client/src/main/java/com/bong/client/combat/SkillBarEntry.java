package com.bong.client.combat;

import java.util.Objects;

/** Single 1-9 combat skill bar binding. */
public final class SkillBarEntry {
    public enum Kind { ITEM, SKILL }

    private final Kind kind;
    private final String id;
    private final String displayName;
    private final int castDurationMs;
    private final int cooldownMs;
    private final String iconTexture;

    public SkillBarEntry(
        Kind kind,
        String id,
        String displayName,
        int castDurationMs,
        int cooldownMs,
        String iconTexture
    ) {
        this.kind = Objects.requireNonNull(kind, "kind");
        this.id = Objects.requireNonNull(id, "id");
        this.displayName = displayName == null ? "" : displayName;
        this.castDurationMs = Math.max(0, castDurationMs);
        this.cooldownMs = Math.max(0, cooldownMs);
        this.iconTexture = iconTexture == null ? "" : iconTexture;
    }

    public static SkillBarEntry item(String templateId, String displayName, int castDurationMs, int cooldownMs, String iconTexture) {
        return new SkillBarEntry(Kind.ITEM, templateId, displayName, castDurationMs, cooldownMs, iconTexture);
    }

    public static SkillBarEntry skill(String skillId, String displayName, int castDurationMs, int cooldownMs, String iconTexture) {
        return new SkillBarEntry(Kind.SKILL, skillId, displayName, castDurationMs, cooldownMs, iconTexture);
    }

    public Kind kind() { return kind; }
    public String id() { return id; }
    public String displayName() { return displayName; }
    public int castDurationMs() { return castDurationMs; }
    public int cooldownMs() { return cooldownMs; }
    public String iconTexture() { return iconTexture; }
}
