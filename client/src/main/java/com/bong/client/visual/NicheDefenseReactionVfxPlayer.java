package com.bong.client.visual;

import java.util.List;

public final class NicheDefenseReactionVfxPlayer {
    private NicheDefenseReactionVfxPlayer() {
    }

    public static List<String> reactionIdsFor(String guardianKind, boolean broken) {
        String kind = guardianKind == null || guardianKind.isBlank() ? "unknown" : guardianKind.trim();
        if (broken) {
            return List.of("niche_guardian_broken", "niche_intrusion_ink_mark", kind);
        }
        return List.of("niche_guardian_fatigue", kind);
    }
}
