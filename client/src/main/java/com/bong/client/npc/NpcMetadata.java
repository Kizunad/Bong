package com.bong.client.npc;

public record NpcMetadata(
    int entityId,
    String archetype,
    String realm,
    String factionName,
    String factionRank,
    int reputationToPlayer,
    String displayName,
    String ageBand,
    String greetingText,
    String qiHint
) {
    public NpcMetadata {
        archetype = clean(archetype, "unknown");
        realm = clean(realm, "未知");
        factionName = blankToNull(factionName);
        factionRank = blankToNull(factionRank);
        displayName = clean(displayName, archetype + "·" + realm);
        ageBand = clean(ageBand, "正值壮年");
        greetingText = clean(greetingText, "对方沉默地看着你。");
        qiHint = blankToNull(qiHint);
    }

    public boolean hostile() {
        return reputationToPlayer < -30;
    }

    public boolean tradeCandidate() {
        return !hostile() && ("rogue".equals(archetype) || "commoner".equals(archetype));
    }

    private static String clean(String value, String fallback) {
        if (value == null || value.isBlank()) {
            return fallback;
        }
        return value.trim();
    }

    private static String blankToNull(String value) {
        if (value == null || value.isBlank()) {
            return null;
        }
        return value.trim();
    }
}
