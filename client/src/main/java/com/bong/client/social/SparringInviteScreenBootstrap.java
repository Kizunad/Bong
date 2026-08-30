package com.bong.client.social;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

/**
 * 切磋邀请的礼貌开屏引导（plan-social-v1 + R7 P4 combat-aware consumer）。
 *
 * <p>决策输入（R7 P4 生产接线）：server-authoritative combat snapshot
 * （{@code CombatHudStateStore.authoritativeSnapshot().combatActive()}）。战斗中或被屏挡住时邀请保留在既有
 * domain Store，bootstrap 按 identity 持有 {@code alreadyNotified}：首次阻塞 DEFER_NOTIFY
 * （toast 一次），重复同 identity DEFER_SILENT，新 identity 恢复通知资格；脱战且空屏且
 * 未过 TTL 才 OPEN_SCREEN，先到 TTL 则 EXPIRE（decline）。缺少 authoritative combat
 * producer 时 fail closed——P4 只允许消费 server 下发的 {@code combat_hud_state} 快照，
 * 禁止用 currentScreen/HUD active/本地伤害启发式代替。
 */
public final class SparringInviteScreenBootstrap {
    private static final int TOAST_COLOR = 0xFFAA55;

    private static String lastBlockedToastInviteId = "";
    private static String notifiedInviteId = "";
    private static boolean combatConsumerFailClosedLogged;

    private SparringInviteScreenBootstrap() {
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(SparringInviteScreenBootstrap::onEndClientTick);
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        handleIncomingInvite(client);
    }

    enum ScreenKind {
        NONE,
        MATCHING_SPARRING_INVITE,
        OTHER_SPARRING_INVITE,
        OTHER
    }

    enum CombatState {
        /** P4 fail-closed：缺 authoritative combat producer 时视为战斗未知，禁止开屏。 */
        UNKNOWN,
        NOT_IN_COMBAT,
        IN_COMBAT
    }

    enum Decision {
        NOOP,
        CLOSE_SCREEN,
        DECLINE_EXPIRED,
        DECLINE_EXPIRED_AND_CLOSE_SCREEN,
        OPEN_SCREEN,
        DEFER_NOTIFY,
        DEFER_SILENT
    }

    /** 从 server-authoritative combat snapshot 派生战斗态；缺失 producer → UNKNOWN (fail closed)。 */
    static CombatState combatStateOf() {
        CombatHudState snapshot = CombatHudStateStore.authoritativeSnapshot();
        if (snapshot == null) {
            return CombatState.UNKNOWN;
        }
        return snapshot.combatActive() ? CombatState.IN_COMBAT : CombatState.NOT_IN_COMBAT;
    }

    static Decision decide(SocialStateStore.SparringInvite invite, ScreenKind screenKind, long nowMs) {
        return decide(invite, screenKind, nowMs, combatStateOf(), false);
    }

    static Decision decide(SocialStateStore.SparringInvite invite, ScreenKind screenKind, long nowMs, CombatState combat) {
        boolean alreadyNotified = invite != null && notifiedInviteId.equals(invite.inviteId());
        Decision decision = decide(invite, screenKind, nowMs, combat, alreadyNotified);
        if (invite != null) {
            if (decision == Decision.DEFER_NOTIFY) {
                notifiedInviteId = invite.inviteId();
                notifyBlocked(invite.inviteId());
            } else if (decision == Decision.DEFER_SILENT) {
                notifiedInviteId = invite.inviteId();
            }
        }
        return decision;
    }

    static Decision decide(
        SocialStateStore.SparringInvite invite,
        ScreenKind screenKind,
        long nowMs,
        CombatState combat,
        boolean alreadyNotified
    ) {
        if (invite == null) {
            return screenKind == ScreenKind.MATCHING_SPARRING_INVITE
                || screenKind == ScreenKind.OTHER_SPARRING_INVITE
                ? Decision.CLOSE_SCREEN
                : Decision.NOOP;
        }
        if (invite.expiresAtMs() <= nowMs) {
            return screenKind == ScreenKind.MATCHING_SPARRING_INVITE
                ? Decision.DECLINE_EXPIRED_AND_CLOSE_SCREEN
                : Decision.DECLINE_EXPIRED;
        }
        if (screenKind == ScreenKind.MATCHING_SPARRING_INVITE) {
            return Decision.NOOP;
        }
        if (combat == CombatState.UNKNOWN) {
            failClosed();
            return alreadyNotified ? Decision.DEFER_SILENT : Decision.DEFER_NOTIFY;
        }
        if (combat == CombatState.IN_COMBAT || screenKind != ScreenKind.NONE) {
            return alreadyNotified ? Decision.DEFER_SILENT : Decision.DEFER_NOTIFY;
        }
        return Decision.OPEN_SCREEN;
    }

    static Decision decide(SocialStateStore.SparringInvite invite, Screen current, long nowMs) {
        CombatState combat = combatStateOf();
        boolean alreadyNotified = invite != null && notifiedInviteId.equals(invite.inviteId());
        return decide(invite, screenKindOf(current, invite), nowMs, combat, alreadyNotified);
    }

    private static void handleIncomingInvite(MinecraftClient client) {
        SocialStateStore.SparringInvite invite = SocialStateStore.sparringInvite();
        Screen current = client.currentScreen;
        Decision decision = decide(invite, current, System.currentTimeMillis());
        switch (decision) {
            case CLOSE_SCREEN -> {
                client.setScreen(null);
                lastBlockedToastInviteId = "";
            }
            case DECLINE_EXPIRED, DECLINE_EXPIRED_AND_CLOSE_SCREEN -> {
                boolean settled = SocialStateStore.clearSparringInvite(invite.inviteId());
                if (settled) {
                    ClientRequestSender.sendSparringInviteResponse(invite.inviteId(), false, true);
                    notifyExpired();
                }
                if (decision == Decision.DECLINE_EXPIRED_AND_CLOSE_SCREEN) {
                    client.setScreen(null);
                }
                lastBlockedToastInviteId = "";
                notifiedInviteId = "";
            }
            case OPEN_SCREEN -> {
                client.setScreen(new SparringInviteScreen(invite));
                lastBlockedToastInviteId = "";
                notifiedInviteId = "";
            }
            case DEFER_NOTIFY -> {
                // 首次阻塞（战斗或屏幕占用）：toast 一次并按 identity 记录通知态
                notifiedInviteId = invite.inviteId();
                notifyBlocked(invite.inviteId());
            }
            case DEFER_SILENT -> {
                // 重复阻塞：静默；identity 更新后自动恢复通知资格
                notifiedInviteId = invite.inviteId();
            }
            case NOOP -> {
                if (invite == null) {
                    lastBlockedToastInviteId = "";
                }
            }
        }
    }

    private static ScreenKind screenKindOf(Screen current, SocialStateStore.SparringInvite invite) {
        if (current instanceof SparringInviteScreen screen) {
            return invite != null && screen.inviteId().equals(invite.inviteId())
                ? ScreenKind.MATCHING_SPARRING_INVITE
                : ScreenKind.OTHER_SPARRING_INVITE;
        }
        return current == null ? ScreenKind.NONE : ScreenKind.OTHER;
    }

    /** P4 fail-closed：combat producer 缺失时最多记录一次诊断。 */
    private static void failClosed() {
        if (!combatConsumerFailClosedLogged) {
            combatConsumerFailClosedLogged = true;
            com.bong.client.BongClient.LOGGER.warn(
                "[sparring-invite] P4 fail closed: authoritative combat_hud_state producer missing; "
                    + "invite deferred without opening"
            );
        }
    }

    static void notifyBlocked(String inviteId) {
        if (inviteId == null || inviteId.isBlank() || inviteId.equals(lastBlockedToastInviteId)) {
            return;
        }
        lastBlockedToastInviteId = inviteId;
        BongToast.show("切磋邀请到达 · 关闭当前界面查看", TOAST_COLOR, System.currentTimeMillis(), 4_000L);
    }

    static void notifyExpired() {
        BongToast.show("切磋邀请已过期", TOAST_COLOR, System.currentTimeMillis(), 3_000L);
    }

    public static void clearOnDisconnect() {
        lastBlockedToastInviteId = "";
        notifiedInviteId = "";
    }

    static void resetForTests() {
        clearOnDisconnect();
        combatConsumerFailClosedLogged = false;
    }
}
