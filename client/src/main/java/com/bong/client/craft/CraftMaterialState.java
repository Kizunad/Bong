package com.bong.client.craft;

import java.util.Objects;

/** 单条材料在当前背包快照下的可制作状态。 */
public record CraftMaterialState(
    String templateId,
    int need,
    int have
) {
    public CraftMaterialState {
        Objects.requireNonNull(templateId, "templateId");
        if (need < 0) {
            throw new IllegalArgumentException("need must be >= 0");
        }
        if (have < 0) {
            throw new IllegalArgumentException("have must be >= 0");
        }
    }

    public boolean sufficient() {
        return have >= need;
    }

    public int missing() {
        return Math.max(0, need - have);
    }
}
