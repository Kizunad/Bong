package com.bong.client.hud;

import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.util.RealmLabel;
import net.minecraft.entity.Entity;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.player.PlayerEntity;

import java.util.Locale;
import java.util.Objects;

public final class TargetInfoState {
    public static final long HOLD_MILLIS = 5_000L;
    public static final long FADE_MILLIS = 1_000L;

    public enum Kind {
        NPC,
        MOB,
        PLAYER
    }

    private final Kind kind;
    private final String targetId;
    private final String displayName;
    private final String realm;
    private final double hpRatio;
    private final double qiRatio;
    private final long observedAtMillis;

    private TargetInfoState(
        Kind kind,
        String targetId,
        String displayName,
        String realm,
        double hpRatio,
        double qiRatio,
        long observedAtMillis
    ) {
        this.kind = kind == null ? Kind.MOB : kind;
        this.targetId = normalize(targetId);
        this.displayName = normalize(displayName);
        this.realm = normalize(realm);
        this.hpRatio = clamp01(hpRatio);
        this.qiRatio = clamp01(qiRatio);
        this.observedAtMillis = Math.max(0L, observedAtMillis);
    }

    public static TargetInfoState empty() {
        return new TargetInfoState(Kind.MOB, "", "", "", 0.0, 0.0, 0L);
    }

    public static TargetInfoState create(
        Kind kind,
        String targetId,
        String displayName,
        String realm,
        double hpRatio,
        double qiRatio,
        long observedAtMillis
    ) {
        if (normalize(targetId).isEmpty() && normalize(displayName).isEmpty()) {
            return empty();
        }
        return new TargetInfoState(kind, targetId, displayName, realm, hpRatio, qiRatio, observedAtMillis);
    }

    public static TargetInfoState fromEntity(Entity entity, long observedAtMillis) {
        if (!(entity instanceof LivingEntity living)) {
            return empty();
        }
        Kind kind = living instanceof PlayerEntity ? Kind.PLAYER : Kind.MOB;
        String name = living.getDisplayName() == null
            ? living.getType().getName().getString()
            : living.getDisplayName().getString();
        float maxHealth = Math.max(1.0f, living.getMaxHealth());
        return create(
            kind,
            "entity:" + living.getId(),
            name,
            "",
            living.getHealth() / maxHealth,
            0.0,
            observedAtMillis
        );
    }

    public boolean isEmpty() {
        return targetId.isEmpty() && displayName.isEmpty();
    }

    public boolean activeAt(long nowMillis) {
        return !isEmpty() && ageMillis(nowMillis) < HOLD_MILLIS;
    }

    public int alphaAt(long nowMillis) {
        long age = ageMillis(nowMillis);
        if (age >= HOLD_MILLIS) {
            return 0;
        }
        long fadeStart = HOLD_MILLIS - FADE_MILLIS;
        if (age <= fadeStart) {
            return 255;
        }
        long remaining = HOLD_MILLIS - age;
        return HudTextHelper.clampAlpha((int) Math.round(255.0 * remaining / FADE_MILLIS));
    }

    public boolean revealRealm(PlayerStateViewModel viewer) {
        if (realm.isEmpty()) {
            return false;
        }
        if (kind == Kind.PLAYER) {
            return true;
        }
        int viewerRank = realmRank(viewer == null ? "" : viewer.realm());
        int targetRank = realmRank(realm);
        return viewerRank >= targetRank;
    }

    public String realmText(PlayerStateViewModel viewer) {
        if (realm.isEmpty()) {
            return kind == Kind.MOB ? "" : "???";
        }
        return revealRealm(viewer) ? RealmLabel.displayName(realm) : "???";
    }

    private long ageMillis(long nowMillis) {
        return Math.max(0L, Math.max(0L, nowMillis) - observedAtMillis);
    }

    static int realmRank(String value) {
        String normalized = normalize(value).toLowerCase(Locale.ROOT);
        return switch (normalized) {
            case "awaken", "醒灵" -> 1;
            case "induce", "引气" -> 2;
            case "condense", "凝脉" -> 3;
            case "solidify", "固元" -> 4;
            case "spirit", "通灵" -> 5;
            case "void", "化虚" -> 6;
            default -> 0;
        };
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim();
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }

    public Kind kind() {
        return kind;
    }

    public String targetId() {
        return targetId;
    }

    public String displayName() {
        return displayName;
    }

    public String realm() {
        return realm;
    }

    public double hpRatio() {
        return hpRatio;
    }

    public double qiRatio() {
        return qiRatio;
    }

    public long observedAtMillis() {
        return observedAtMillis;
    }

    @Override
    public boolean equals(Object obj) {
        if (this == obj) return true;
        if (!(obj instanceof TargetInfoState other)) return false;
        return kind == other.kind
            && targetId.equals(other.targetId)
            && displayName.equals(other.displayName)
            && realm.equals(other.realm)
            && Double.compare(hpRatio, other.hpRatio) == 0
            && Double.compare(qiRatio, other.qiRatio) == 0
            && observedAtMillis == other.observedAtMillis;
    }

    @Override
    public int hashCode() {
        return Objects.hash(kind, targetId, displayName, realm, hpRatio, qiRatio, observedAtMillis);
    }
}
