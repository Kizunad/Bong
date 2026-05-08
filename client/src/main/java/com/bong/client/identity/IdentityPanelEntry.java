package com.bong.client.identity;

import java.util.List;
import java.util.Objects;

/**
 * client 侧 plan-identity-v1 §7 IdentityPanelEntryV1 镜像。
 *
 * <p>对应 schema：{@code agent/packages/schema/src/identity.ts -> IdentityPanelEntryV1}
 * + Rust：{@code server/src/schema/identity.rs -> IdentityPanelEntryV1}。
 */
public final class IdentityPanelEntry {
    private final int identityId;
    private final String displayName;
    private final int reputationScore;
    private final boolean frozen;
    private final List<String> revealedTagKinds;

    public IdentityPanelEntry(
            int identityId,
            String displayName,
            int reputationScore,
            boolean frozen,
            List<String> revealedTagKinds) {
        this.identityId = identityId;
        this.displayName = Objects.requireNonNullElse(displayName, "");
        this.reputationScore = reputationScore;
        this.frozen = frozen;
        this.revealedTagKinds = revealedTagKinds == null ? List.of() : List.copyOf(revealedTagKinds);
    }

    public int identityId() {
        return identityId;
    }

    public String displayName() {
        return displayName;
    }

    public int reputationScore() {
        return reputationScore;
    }

    public boolean frozen() {
        return frozen;
    }

    public List<String> revealedTagKinds() {
        return revealedTagKinds;
    }
}
