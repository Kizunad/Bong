package com.bong.client.npc;

/**
 * NPC 交易报价（由 {@code bong:npc_metadata} 下发）。
 */
public record NpcTradeOffer(
    String templateId,
    String displayName,
    int count,
    int priceBoneCoins
) {
    public NpcTradeOffer {
        templateId = templateId == null ? "" : templateId;
        displayName = displayName == null || displayName.isBlank() ? templateId : displayName;
        count = Math.max(0, count);
        priceBoneCoins = Math.max(0, priceBoneCoins);
    }
}
