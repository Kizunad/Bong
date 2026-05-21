package com.bong.client.npc;

import java.util.List;
import java.util.Map;

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
    String qiHint,
    double hpRatio,
    double qiRatio,
    Map<String, NpcEquipSlotData> equipment,
    List<NpcTechniqueData> techniques,
    List<NpcTradeOffer> tradeOffers
) {
    /** Backward-compatible 10-param constructor (old server without hp/qi/equip/tech/trade). */
    public NpcMetadata(
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
        this(
            entityId,
            archetype,
            realm,
            factionName,
            factionRank,
            reputationToPlayer,
            displayName,
            ageBand,
            greetingText,
            qiHint,
            1.0,
            0.0,
            Map.of(),
            List.of(),
            List.of()
        );
    }

    /** Backward-compatible 12-param constructor (old server without equip/tech/trade). */
    public NpcMetadata(
        int entityId,
        String archetype,
        String realm,
        String factionName,
        String factionRank,
        int reputationToPlayer,
        String displayName,
        String ageBand,
        String greetingText,
        String qiHint,
        double hpRatio,
        double qiRatio
    ) {
        this(
            entityId,
            archetype,
            realm,
            factionName,
            factionRank,
            reputationToPlayer,
            displayName,
            ageBand,
            greetingText,
            qiHint,
            hpRatio,
            qiRatio,
            Map.of(),
            List.of(),
            List.of()
        );
    }

    public NpcMetadata {
        archetype = clean(archetype, "unknown");
        realm = clean(realm, "未知");
        factionName = blankToNull(factionName);
        factionRank = blankToNull(factionRank);
        displayName = clean(displayName, archetype + "·" + realm);
        ageBand = clean(ageBand, "正值壮年");
        greetingText = clean(greetingText, "对方沉默地看着你。");
        qiHint = blankToNull(qiHint);
        hpRatio = clamp01(hpRatio);
        qiRatio = clamp01(qiRatio);
        equipment = equipment == null ? Map.of() : Map.copyOf(equipment);
        techniques = techniques == null ? List.of() : List.copyOf(techniques);
        tradeOffers = tradeOffers == null ? List.of() : List.copyOf(tradeOffers);
    }

    public boolean hostile() {
        return reputationToPlayer < -30;
    }

    /**
     * Data-driven trade candidacy: not hostile and has trade offers available.
     * Replaces the old hardcoded archetype check.
     */
    public boolean tradeCandidate() {
        return !hostile() && tradeOffers != null && !tradeOffers.isEmpty();
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

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
