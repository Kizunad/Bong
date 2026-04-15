package com.bong.client.combat;

/**
 * Immutable set of unlocked defensive styles (§3.4 / §11.1). Drives conditional
 * render gates; unlocked-but-unused still renders indicators, unequipped
 * styles are &ldquo;not rendered at all&rdquo; (§1.6).
 */
public final class UnlockedStyles {
    private static final UnlockedStyles NONE = new UnlockedStyles(false, false, false);
    private static final UnlockedStyles ALL = new UnlockedStyles(true, true, true);

    private final boolean jiemai;
    private final boolean tishi;
    private final boolean jueling;

    private UnlockedStyles(boolean jiemai, boolean tishi, boolean jueling) {
        this.jiemai = jiemai;
        this.tishi = tishi;
        this.jueling = jueling;
    }

    public static UnlockedStyles none() { return NONE; }
    public static UnlockedStyles all() { return ALL; }

    public static UnlockedStyles of(boolean jiemai, boolean tishi, boolean jueling) {
        if (!jiemai && !tishi && !jueling) return NONE;
        if (jiemai && tishi && jueling) return ALL;
        return new UnlockedStyles(jiemai, tishi, jueling);
    }

    public boolean jiemai() { return jiemai; }
    public boolean tishi() { return tishi; }
    public boolean jueling() { return jueling; }
}
