package com.bong.client.movement;

import com.bong.client.combat.inspect.TechniquesListPanel;

/** 身法可用性复用服务端功法快照，不维护第二份解锁状态。 */
public final class DashSkill {
    public static final String ID = "movement.dash";

    private DashSkill() {}

    public static boolean learned() {
        return TechniquesListPanel.snapshot().stream()
            .anyMatch(technique -> ID.equals(technique.id()) && technique.active());
    }
}
