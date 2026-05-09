package com.bong.client.combat;

import com.bong.client.network.ClientRequestSender;

import net.minecraft.client.MinecraftClient;
import net.minecraft.util.hit.EntityHitResult;

/** Pure routing logic for 1-9 hotbar key presses; the mixin delegates here. */
public final class SkillBarKeyRouter {
    public enum RouteResult { NOOP, PASS_THROUGH, CAST_SENT, CONTAINER_SWITCH_SENT, COOLDOWN_BLOCKED, SAME_CAST_IGNORED }

    private SkillBarKeyRouter() {
    }

    public static boolean shouldCancelHotbarKey(int slot) {
        RouteResult result = route(slot, System.currentTimeMillis(), SkillBarKeyRouter::sendCastWithCrosshairTarget);
        return result == RouteResult.CAST_SENT
            || result == RouteResult.COOLDOWN_BLOCKED
            || result == RouteResult.SAME_CAST_IGNORED;
    }

    public static boolean shouldCancelAnqiContainerKey() {
        return routeAnqiContainerSwitch(ClientRequestSender::sendAnqiContainerSwitch)
            == RouteResult.CONTAINER_SWITCH_SENT;
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

    public static RouteResult routeAnqiContainerSwitch(Runnable switchSender) {
        SkillBarConfig config = SkillBarStore.snapshot();
        for (int i = 0; i < SkillBarConfig.SLOT_COUNT; i++) {
            SkillBarEntry entry = config.slot(i);
            if (entry != null && entry.kind() == SkillBarEntry.Kind.SKILL && entry.id().startsWith("anqi.")) {
                switchSender.run();
                return RouteResult.CONTAINER_SWITCH_SENT;
            }
        }
        return RouteResult.PASS_THROUGH;
    }

    private static void sendCastWithCrosshairTarget(int slot) {
        ClientRequestSender.sendSkillBarCast(slot, crosshairEntityTarget());
    }

    static String crosshairEntityTarget() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || !(client.crosshairTarget instanceof EntityHitResult hit)) {
            return null;
        }
        return "entity:" + hit.getEntity().getId();
    }
}
