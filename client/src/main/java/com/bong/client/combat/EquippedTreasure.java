package com.bong.client.combat;

public record EquippedTreasure(
    String slot,
    long instanceId,
    String templateId,
    String displayName
) {}
