package com.bong.client.botany;

/**
 * Compatibility-only local botany skill mirror.
 *
 * <p>TODO(plan-skill-v1): migrate this transitional view to SkillSetStore-derived
 * herbalism data once the shared skill system lands. Botany must not grow its own
 * long-term skill subsystem.
 */
public record BotanySkillViewModel(
    int level,
    long xp,
    long xpToNextLevel,
    int autoUnlockLevel
) {
    private static final int DEFAULT_AUTO_UNLOCK_LEVEL = 3;
    private static final BotanySkillViewModel DEFAULT = new BotanySkillViewModel(0, 0L, 100L, DEFAULT_AUTO_UNLOCK_LEVEL);

    public BotanySkillViewModel {
        level = Math.max(0, level);
        xp = Math.max(0L, xp);
        xpToNextLevel = Math.max(1L, xpToNextLevel);
        autoUnlockLevel = Math.max(1, autoUnlockLevel);
    }

    public static BotanySkillViewModel defaultView() {
        return DEFAULT;
    }

    public static BotanySkillViewModel create(int level, long xp, long xpToNextLevel, int autoUnlockLevel) {
        return new BotanySkillViewModel(level, xp, xpToNextLevel, autoUnlockLevel);
    }

    public boolean autoUnlocked() {
        return level >= autoUnlockLevel;
    }

    public double progressRatio() {
        if (xpToNextLevel <= 0L) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, xp / (double) xpToNextLevel));
    }
}
