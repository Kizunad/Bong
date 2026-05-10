package com.bong.client.environment;

public final class ActiveEmitter {
    private final String key;
    private final String zoneId;
    private EnvironmentEffect effect;
    private EmitterBehavior behavior;
    private long generation;
    private float alpha;
    private boolean fadingOut;
    private boolean inRadius;

    ActiveEmitter(String key, String zoneId, EnvironmentEffect effect, EmitterBehavior behavior, long generation) {
        this.key = key;
        this.zoneId = zoneId;
        this.effect = effect;
        this.behavior = behavior;
        this.generation = generation;
        this.alpha = 0.0f;
        this.fadingOut = false;
        this.inRadius = false;
    }

    void refresh(EnvironmentEffect nextEffect, EmitterBehavior nextBehavior, long nextGeneration) {
        this.effect = nextEffect;
        this.behavior = nextBehavior;
        this.generation = nextGeneration;
        this.fadingOut = false;
    }

    void markFadingOut() {
        this.fadingOut = true;
    }

    boolean advanceFade() {
        return advanceFade(true);
    }

    boolean advanceFade(boolean nextInRadius) {
        this.inRadius = nextInRadius;
        if (fadingOut || !nextInRadius) {
            alpha -= 1.0f / Math.max(1, behavior.fadeOutTicks(effect));
            if (alpha <= 0.0f) {
                alpha = 0.0f;
                return !fadingOut;
            }
            return true;
        }
        alpha += 1.0f / Math.max(1, behavior.fadeInTicks(effect));
        alpha = Math.min(1.0f, alpha);
        return true;
    }

    public String key() {
        return key;
    }

    public String zoneId() {
        return zoneId;
    }

    public EnvironmentEffect effect() {
        return effect;
    }

    public EmitterBehavior behavior() {
        return behavior;
    }

    public long generation() {
        return generation;
    }

    public float alpha() {
        return alpha;
    }

    public boolean fadingOut() {
        return fadingOut;
    }

    public boolean inRadius() {
        return inRadius;
    }
}
