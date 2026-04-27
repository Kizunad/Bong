package com.bong.client.combat;

import com.bong.client.network.ClientRequestSender;

/** Pure routing logic for 1-9 hotbar key presses; the mixin delegates here. */
public final class SkillBarKeyRouter {
    public enum RouteResult { NOOP, PASS_THROUGH, CAST_SENT, COOLDOWN_BLOCKED, SAME_CAST_IGNORED }

    private SkillBarKeyRouter() {
    }

    public static boolean shouldCancelHotbarKey(int slot) {
        RouteResult result = route(slot, System.currentTimeMillis(), ClientRequestSender::sendSkillBarCast);
        return result == RouteResult.CAST_SENT
            || result == RouteResult.COOLDOWN_BLOCKED
            || result == RouteResult.SAME_CAST_IGNORED;
    }

    public static RouteResult route(int slot, long nowMs, java.util.function.IntConsumer castSender) {
        if (slot < 0 || slot >= SkillBarConfig.SLOT_COUNT) return RouteResult.NOOP;
        SkillBarConfig config = SkillBarStore.snapshot();
        SkillBarEntry entry = config.slot(slot);
        if (entry == null) return RouteResult.PASS_THROUGH;
        if (entry.kind() == SkillBarEntry.Kind.ITEM) return RouteResult.PASS_THROUGH;
        if (config.isOnCooldown(slot, nowMs)) return RouteResult.COOLDOWN_BLOCKED;

        CastState current = CastStateStore.snapshot();
        if (current.isCasting()) {
            if (current.slot() == slot) return RouteResult.SAME_CAST_IGNORED;
            CastStateStore.interrupt(CastOutcome.USER_CANCEL, nowMs);
        }
        CastStateStore.beginSkillBarCast(slot, entry.castDurationMs(), nowMs);
        castSender.accept(slot);
        return RouteResult.CAST_SENT;
    }
}
