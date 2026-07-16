package com.bong.client.social;

import com.bong.client.hud.BongToast;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

public final class SparringInviteScreenBootstrap {
    private static final int TOAST_COLOR = 0xFFAA55;

    private static String lastBlockedToastInviteId = "";

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

    enum Decision {
        NOOP,
        CLOSE_SCREEN,
        DECLINE_EXPIRED,
        DECLINE_EXPIRED_AND_CLOSE_SCREEN,
        OPEN_SCREEN,
        BLOCKED_TOAST
    }

    static Decision decide(SocialStateStore.SparringInvite invite, ScreenKind screenKind, long nowMs) {
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
        return screenKind == ScreenKind.NONE ? Decision.OPEN_SCREEN : Decision.BLOCKED_TOAST;
    }

    static Decision decide(SocialStateStore.SparringInvite invite, Screen current, long nowMs) {
        return decide(invite, screenKindOf(current, invite), nowMs);
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
            }
            case OPEN_SCREEN -> {
                client.setScreen(new SparringInviteScreen(invite));
                lastBlockedToastInviteId = "";
            }
            case BLOCKED_TOAST -> notifyBlocked(invite.inviteId());
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
    }

    static void resetForTests() {
        clearOnDisconnect();
    }
}
