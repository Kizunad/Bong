package com.bong.client.combat.store;

/** 仅持有终结 payload 的属性，不读取重生后的实时 HUD。 */
public record TerminationSummary(
    String characterName, String realm, int deathCount,
    Double yearsLived, Double qiMax, Double healthMax,
    Integer meridiansOpen, Integer techniquesLearned
) {
    public static final TerminationSummary EMPTY = new TerminationSummary("", "", 0, null, null, null, null, null);

    public TerminationSummary {
        characterName = characterName == null ? "" : characterName;
        realm = realm == null ? "" : realm;
        deathCount = Math.max(0, deathCount);
    }
}
