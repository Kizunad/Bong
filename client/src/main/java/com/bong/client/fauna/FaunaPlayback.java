package com.bong.client.fauna;

/** 单实体的形态与动作状态；无 MC 实例依赖，可验证消息与动作结束的切换。 */
public final class FaunaPlayback {
    private final FaunaVisualKind kind;
    private final FaunaActionAnimation action = new FaunaActionAnimation();
    private FaunaAnimations.Profile profile;
    private int revision;
    private Boolean disguised;
    private boolean block;

    public FaunaPlayback(FaunaVisualKind kind) {
        this.kind = kind;
        profile = kind.animations();
    }

    public boolean trigger(String name, int ticks) {
        if (name == null || ticks <= 0) return false;
        // 先查当前形态，短名 idle/walk 不会意外把核心或飞行姿切回主模型。
        FaunaAnimations.Profile selected = profile;
        FaunaAnimations.Clip clip = profile.find(name);
        if (clip == null) {
            for (var candidate : FaunaAnimations.profiles(kind)) {
                clip = candidate.find(name);
                if (clip != null) {
                    selected = candidate;
                    break;
                }
            }
        }
        if (clip == null) return false;
        profile = selected;
        action.trigger(clip.name(), ticks);
        revision++;
        return true;
    }

    public void tick() {
        String previous = action.currentAnim();
        action.tick();
        if (previous != null && !action.hasAction()) {
            String name = FaunaAnimations.shortName(previous);
            if (kind == FaunaVisualKind.FUYU_VULTURE) {
                if (name.equals("land")) profile = kind.animations();
                if (name.equals("unfold")) profile = FaunaAnimations.load("fuyu_vulture_flight");
            }
            if (kind == FaunaVisualKind.ASH_SPIDER && name.equals("fold") && Boolean.TRUE.equals(disguised)) {
                block = true;
            }
            revision++;
        }
    }

    /** 初次全量同步直接显示方块；只有实际状态边沿才播放折叠/暴起。 */
    public void syncSpiderDisguise(int entityId) {
        spiderDisguise(com.bong.client.spider.SpiderDisguiseHandler.isDisguised(entityId),
            com.bong.client.spider.SpiderDisguiseHandler.isRevealed(entityId));
    }

    public void spiderDisguise(boolean value, boolean revealed) {
        if (kind != FaunaVisualKind.ASH_SPIDER) return;
        if (disguised == null) {
            disguised = value;
            block = value;
        } else if (disguised != value) {
            disguised = value;
            if (value) {
                block = false;
                play("fold");
            } else {
                block = false;
                if (revealed) play("ambush_burst");
            }
        }
    }

    public boolean play(String name) {
        var clip = profile.find(name);
        return clip != null && trigger(clip.name(), clip.ticks());
    }

    public FaunaAnimations.Clip current(float speed) {
        if (action.hasAction()) return profile.find(action.currentAnim());
        if (kind == FaunaVisualKind.HORSE) {
            // 马的四种步态都有独立落蹄节奏；中速不直接跳到袭步。
            if (speed >= 0.27f) return profile.find("gallop");
            if (speed >= 0.18f) return profile.find("canter");
            if (speed >= 0.09f) return profile.find("trot");
            return speed >= 0.012f ? profile.walk() : profile.idle();
        }
        if (profile.run() != null && speed >= 0.14f) return profile.run();
        if (profile.walk() != null && speed >= 0.012f) return profile.walk();
        return profile.idle();
    }

    public FaunaAnimations.Profile profile() { return profile; }
    public String actionName() { return action.currentAnim(); }
    public int remainingTicks() { return action.remainingTicks(); }
    public int revision() { return revision; }
    public boolean blockDisguise() { return block; }
}
