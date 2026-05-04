package com.bong.client.social;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

public final class NicheGuardianPanel {
    private NicheGuardianPanel() {
    }

    public static List<String> buildLines() {
        ArrayList<String> lines = new ArrayList<>();
        NicheGuardianStore.guardianStatuses().values().stream()
            .sorted(Comparator.comparing(NicheGuardianStore.GuardianStatus::guardianKind))
            .forEach(status -> lines.add(status.guardianKind()
                + " x" + status.chargesRemaining()
                + (status.broken() ? " broken" : "")));
        if (lines.isEmpty()) {
            lines.add("无守家载体");
        }
        NicheGuardianStore.intrusionAlerts().stream()
            .limit(3)
            .forEach(alert -> lines.add("龛侵 " + alert.intruderId() + " 物品 " + alert.itemsTaken().size()));
        return List.copyOf(lines);
    }
}
