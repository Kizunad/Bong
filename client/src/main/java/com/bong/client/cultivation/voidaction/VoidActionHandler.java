package com.bong.client.cultivation.voidaction;

import com.bong.client.network.ClientRequestSender;

import java.util.List;

public final class VoidActionHandler {
    private static final double DEFAULT_BARRIER_RADIUS = 24.0;

    private VoidActionHandler() {}

    public static boolean dispatchSuppressTsy(String zoneId, long nowTick) {
        if (!canDispatch(VoidActionKind.SUPPRESS_TSY, nowTick)) return false;
        ClientRequestSender.sendVoidActionSuppressTsy(zoneId);
        VoidActionStore.markDispatched(VoidActionKind.SUPPRESS_TSY, nowTick);
        return true;
    }

    public static boolean dispatchExplodeZone(String zoneId, long nowTick) {
        if (!canDispatch(VoidActionKind.EXPLODE_ZONE, nowTick)) return false;
        ClientRequestSender.sendVoidActionExplodeZone(zoneId);
        VoidActionStore.markDispatched(VoidActionKind.EXPLODE_ZONE, nowTick);
        return true;
    }

    public static boolean dispatchBarrier(String zoneId, double x, double y, double z, long nowTick) {
        return dispatchBarrier(zoneId, x, y, z, DEFAULT_BARRIER_RADIUS, nowTick);
    }

    public static boolean dispatchBarrier(String zoneId, double x, double y, double z, double radius, long nowTick) {
        if (!canDispatch(VoidActionKind.BARRIER, nowTick)) return false;
        ClientRequestSender.sendVoidActionBarrier(zoneId, x, y, z, radius);
        VoidActionStore.markDispatched(VoidActionKind.BARRIER, nowTick);
        return true;
    }

    public static boolean dispatchLegacyAssign(
        String inheritorId,
        List<Long> itemInstanceIds,
        String message,
        long nowTick
    ) {
        if (!canDispatch(VoidActionKind.LEGACY_ASSIGN, nowTick)) return false;
        ClientRequestSender.sendVoidActionLegacyAssign(inheritorId, itemInstanceIds, message);
        VoidActionStore.markDispatched(VoidActionKind.LEGACY_ASSIGN, nowTick);
        return true;
    }

    private static boolean canDispatch(VoidActionKind kind, long nowTick) {
        return VoidActionStore.snapshot().ready(kind, nowTick);
    }
}
