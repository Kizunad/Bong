package com.bong.client.combat.juice;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class HitStopController {
    private static final Map<String, Freeze> FREEZES = new ConcurrentHashMap<>();

    private HitStopController() {
    }

    public static List<Freeze> request(String attackerUuid, String targetUuid, CombatJuiceProfile profile, long nowMs) {
        if (profile == null || profile.hitStopTicks() <= 0) {
            return List.of();
        }
        List<Freeze> created = new ArrayList<>(2);
        if (targetUuid != null && !targetUuid.isBlank()) {
            created.add(put(targetUuid, Role.DEFENDER, profile.hitStopTicks(), nowMs));
        }
        if (attackerUuid != null && !attackerUuid.isBlank()) {
            int attackerTicks = Math.max(1, (int) Math.floor(profile.hitStopTicks() * 0.5));
            created.add(put(attackerUuid, Role.ATTACKER, attackerTicks, nowMs));
        }
        return List.copyOf(created);
    }

    public static boolean isFrozen(String entityUuid, long nowMs) {
        return remainingTicks(entityUuid, nowMs) > 0;
    }

    public static int remainingTicks(String entityUuid, long nowMs) {
        if (entityUuid == null || entityUuid.isBlank()) {
            return 0;
        }
        Freeze freeze = FREEZES.get(entityUuid);
        if (freeze == null) {
            return 0;
        }
        long remainingMs = freeze.endsAtMs() - nowMs;
        if (remainingMs <= 0L) {
            FREEZES.remove(entityUuid, freeze);
            return 0;
        }
        return (int) Math.ceil(remainingMs / 50.0);
    }

    public static List<Freeze> activeFreezes(long nowMs) {
        tick(nowMs);
        return List.copyOf(FREEZES.values());
    }

    public static void tick(long nowMs) {
        FREEZES.entrySet().removeIf(entry -> entry.getValue().endsAtMs() <= nowMs);
    }

    public static void resetForTests() {
        FREEZES.clear();
    }

    private static Freeze put(String entityUuid, Role role, int ticks, long nowMs) {
        Freeze freeze = new Freeze(entityUuid, role, ticks, nowMs, nowMs + ticks * 50L);
        FREEZES.merge(entityUuid, freeze, (oldFreeze, newFreeze) ->
            oldFreeze.endsAtMs() >= newFreeze.endsAtMs() ? oldFreeze : newFreeze
        );
        return FREEZES.get(entityUuid);
    }

    public enum Role {
        ATTACKER,
        DEFENDER
    }

    public record Freeze(String entityUuid, Role role, int requestedTicks, long startedAtMs, long endsAtMs) {
    }
}
