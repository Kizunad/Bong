package com.bong.client.social;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class NicheGuardianStore {
    private static final int MAX_ALERTS = 16;

    private static volatile List<NicheIntrusionAlert> intrusionAlerts = List.of();
    private static volatile Map<String, GuardianStatus> guardianStatuses = Map.of();

    private NicheGuardianStore() {
    }

    public static List<NicheIntrusionAlert> intrusionAlerts() {
        return intrusionAlerts;
    }

    public static Map<String, GuardianStatus> guardianStatuses() {
        return guardianStatuses;
    }

    public static synchronized void recordIntrusion(NicheIntrusionAlert alert) {
        if (alert == null) return;
        ArrayList<NicheIntrusionAlert> next = new ArrayList<>(intrusionAlerts.size() + 1);
        next.add(alert);
        next.addAll(intrusionAlerts);
        if (next.size() > MAX_ALERTS) {
            next.subList(MAX_ALERTS, next.size()).clear();
        }
        intrusionAlerts = List.copyOf(next);
    }

    public static synchronized void recordFatigue(String guardianKind, int chargesRemaining) {
        String key = normalizedKind(guardianKind);
        if (key.isBlank()) return;
        LinkedHashMap<String, GuardianStatus> next = new LinkedHashMap<>(guardianStatuses);
        next.put(key, new GuardianStatus(key, Math.max(0, chargesRemaining), false,
            System.currentTimeMillis()));
        guardianStatuses = Map.copyOf(next);
    }

    public static synchronized void recordBroken(String guardianKind, String intruderId) {
        String key = normalizedKind(guardianKind);
        if (key.isBlank()) return;
        LinkedHashMap<String, GuardianStatus> next = new LinkedHashMap<>(guardianStatuses);
        next.put(key, new GuardianStatus(key, 0, true, System.currentTimeMillis()));
        guardianStatuses = Map.copyOf(next);
        recordIntrusion(new NicheIntrusionAlert(List.of(), fallback(intruderId), 0.0, System.currentTimeMillis()));
    }

    public static void resetForTests() {
        intrusionAlerts = List.of();
        guardianStatuses = Map.of();
    }

    private static String normalizedKind(String guardianKind) {
        return guardianKind == null ? "" : guardianKind.trim();
    }

    private static String fallback(String value) {
        return value == null || value.isBlank() ? "unknown" : value.trim();
    }

    public record NicheIntrusionAlert(List<Long> itemsTaken, String intruderId, double taintDelta, long receivedAtMs) {
        public NicheIntrusionAlert {
            itemsTaken = List.copyOf(itemsTaken == null ? List.of() : itemsTaken);
            intruderId = fallback(intruderId);
            taintDelta = Math.max(0.0, Math.min(1.0, taintDelta));
        }
    }

    public record GuardianStatus(String guardianKind, int chargesRemaining, boolean broken, long updatedAtMs) {
    }
}
