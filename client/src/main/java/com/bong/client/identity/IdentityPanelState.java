package com.bong.client.identity;

import java.util.List;
import java.util.Objects;
import java.util.Optional;

/**
 * client 侧 plan-identity-v1 §7 IdentityPanelStateV1 镜像。
 *
 * <p>由 server CustomPayload {@code bong:identity_panel_state} 推送，
 * {@link IdentityPanelStateStore} 缓存并通知 UI / HUD 重新渲染。
 */
public final class IdentityPanelState {
    private static final IdentityPanelState EMPTY =
            new IdentityPanelState(0, 0L, 0L, List.of());

    private final int activeIdentityId;
    private final long lastSwitchTick;
    private final long cooldownRemainingTicks;
    private final List<IdentityPanelEntry> identities;

    public IdentityPanelState(
            int activeIdentityId,
            long lastSwitchTick,
            long cooldownRemainingTicks,
            List<IdentityPanelEntry> identities) {
        this.activeIdentityId = activeIdentityId;
        this.lastSwitchTick = lastSwitchTick;
        this.cooldownRemainingTicks = cooldownRemainingTicks;
        this.identities = identities == null ? List.of() : List.copyOf(identities);
    }

    public static IdentityPanelState empty() {
        return EMPTY;
    }

    public int activeIdentityId() {
        return activeIdentityId;
    }

    public long lastSwitchTick() {
        return lastSwitchTick;
    }

    public long cooldownRemainingTicks() {
        return cooldownRemainingTicks;
    }

    public List<IdentityPanelEntry> identities() {
        return identities;
    }

    /** 当前 active identity 的 entry（若 active id 在列表里）。 */
    public Optional<IdentityPanelEntry> activeEntry() {
        for (IdentityPanelEntry entry : identities) {
            if (entry.identityId() == activeIdentityId) {
                return Optional.of(entry);
            }
        }
        return Optional.empty();
    }

    /** 切换冷却是否已过（cooldown_remaining_ticks == 0）。 */
    public boolean cooldownPassed() {
        return cooldownRemainingTicks == 0L;
    }

    @Override
    public boolean equals(Object obj) {
        if (this == obj) {
            return true;
        }
        if (!(obj instanceof IdentityPanelState other)) {
            return false;
        }
        return activeIdentityId == other.activeIdentityId
                && lastSwitchTick == other.lastSwitchTick
                && cooldownRemainingTicks == other.cooldownRemainingTicks
                && Objects.equals(identities, other.identities);
    }

    @Override
    public int hashCode() {
        return Objects.hash(activeIdentityId, lastSwitchTick, cooldownRemainingTicks, identities);
    }
}
