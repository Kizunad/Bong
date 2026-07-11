package com.bong.client.social;

import com.bong.client.hud.BongToast;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SparringInviteScreenBootstrapTest {
    @AfterEach
    void reset() {
        SparringInviteScreenBootstrap.resetForTests();
        BongToast.resetForTests();
    }

    private static SocialStateStore.SparringInvite invite(String id, long expiresAtMs) {
        return new SocialStateStore.SparringInvite(
            id,
            "char:initiator",
            "char:target",
            "凝脉",
            "气息相试",
            "点到为止",
            expiresAtMs
        );
    }

    @Test
    void noInviteLeavesUnrelatedScreenAlone() {
        assertEquals(
            SparringInviteScreenBootstrap.Decision.NOOP,
            SparringInviteScreenBootstrap.decide(
                null,
                SparringInviteScreenBootstrap.ScreenKind.OTHER,
                1_000L
            )
        );
    }

    @Test
    void noInviteClosesAnyLingeringSparringScreen() {
        for (SparringInviteScreenBootstrap.ScreenKind kind : new SparringInviteScreenBootstrap.ScreenKind[] {
            SparringInviteScreenBootstrap.ScreenKind.MATCHING_SPARRING_INVITE,
            SparringInviteScreenBootstrap.ScreenKind.OTHER_SPARRING_INVITE
        }) {
            assertEquals(
                SparringInviteScreenBootstrap.Decision.CLOSE_SCREEN,
                SparringInviteScreenBootstrap.decide(null, kind, 1_000L),
                "store 清空后遗留切磋屏必须关闭，kind=" + kind
            );
        }
    }

    @Test
    void expiredInviteDeclinesWithoutClosingDifferentScreen() {
        SocialStateStore.SparringInvite expired = invite("expired", 1_000L);
        for (SparringInviteScreenBootstrap.ScreenKind kind : SparringInviteScreenBootstrap.ScreenKind.values()) {
            SparringInviteScreenBootstrap.Decision expected =
                kind == SparringInviteScreenBootstrap.ScreenKind.MATCHING_SPARRING_INVITE
                    ? SparringInviteScreenBootstrap.Decision.DECLINE_EXPIRED_AND_CLOSE_SCREEN
                    : SparringInviteScreenBootstrap.Decision.DECLINE_EXPIRED;
            assertEquals(
                expected,
                SparringInviteScreenBootstrap.decide(expired, kind, 1_000L),
                "过期邀请只可关闭 identity 匹配的邀请屏，kind=" + kind
            );
        }
    }

    @Test
    void justBeforeExpiryDoesNotDeclineEarly() {
        assertNotEquals(
            SparringInviteScreenBootstrap.Decision.DECLINE_EXPIRED,
            SparringInviteScreenBootstrap.decide(
                invite("active", 1_001L),
                SparringInviteScreenBootstrap.ScreenKind.NONE,
                1_000L
            )
        );
    }

    @Test
    void activeInviteOpensOnlyWhenNoScreen() {
        SocialStateStore.SparringInvite active = invite("active", 5_000L);
        assertEquals(
            SparringInviteScreenBootstrap.Decision.OPEN_SCREEN,
            SparringInviteScreenBootstrap.decide(active, SparringInviteScreenBootstrap.ScreenKind.NONE, 1_000L)
        );
        assertEquals(
            SparringInviteScreenBootstrap.Decision.BLOCKED_TOAST,
            SparringInviteScreenBootstrap.decide(
                active,
                SparringInviteScreenBootstrap.ScreenKind.OTHER_SPARRING_INVITE,
                1_000L
            ),
            "不同 inviteId 的切磋屏也属于占用态，不能被新邀请强制替换"
        );
        assertEquals(
            SparringInviteScreenBootstrap.Decision.NOOP,
            SparringInviteScreenBootstrap.decide(
                active,
                SparringInviteScreenBootstrap.ScreenKind.MATCHING_SPARRING_INVITE,
                1_000L
            )
        );
    }

    @Test
    void activeInviteBlockedByOtherScreenNeverOpensScreen() {
        assertEquals(
            SparringInviteScreenBootstrap.Decision.BLOCKED_TOAST,
            SparringInviteScreenBootstrap.decide(
                invite("active", 5_000L),
                SparringInviteScreenBootstrap.ScreenKind.OTHER,
                1_000L
            ),
            "本 bug 的回归锁：其他 GUI 打开时只能提示，不能 OPEN_SCREEN 抢屏"
        );
    }

    @Test
    void actualSparringScreenIdentityControlsSameAndDifferentTransitions() {
        SocialStateStore.SparringInvite first = invite("first", 5_000L);
        SocialStateStore.SparringInvite second = invite("second", 6_000L);

        assertEquals(
            SparringInviteScreenBootstrap.Decision.NOOP,
            SparringInviteScreenBootstrap.decide(first, new SparringInviteScreen(first), 1_000L),
            "真实 screen identity 匹配时必须保持当前邀请屏"
        );
        assertEquals(
            SparringInviteScreenBootstrap.Decision.BLOCKED_TOAST,
            SparringInviteScreenBootstrap.decide(second, new SparringInviteScreen(first), 1_000L),
            "真实 screen identity 不同时只能阻塞提示，不能替换当前邀请屏"
        );
    }

    @Test
    void blockedToastIsVisibleAndDeduplicatedPerInviteId() {
        SparringInviteScreenBootstrap.notifyBlocked("invite-1");
        assertFalse(BongToast.current(System.currentTimeMillis()).isEmpty());

        BongToast.resetForTests();
        SparringInviteScreenBootstrap.notifyBlocked("invite-1");
        assertTrue(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "同一 invite 每 tick 只能提示一次"
        );

        SparringInviteScreenBootstrap.notifyBlocked("invite-2");
        assertFalse(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "新的 inviteId 必须重新提示"
        );
    }

    @Test
    void blockedToastIgnoresInvalidIdAndResetRestoresNotification() {
        SparringInviteScreenBootstrap.notifyBlocked("   ");
        assertTrue(BongToast.current(System.currentTimeMillis()).isEmpty());

        SparringInviteScreenBootstrap.notifyBlocked("invite-1");
        BongToast.resetForTests();
        SparringInviteScreenBootstrap.resetForTests();
        SparringInviteScreenBootstrap.notifyBlocked("invite-1");
        assertFalse(BongToast.current(System.currentTimeMillis()).isEmpty());
    }

    @Test
    void expiredToastExplainsOutcome() {
        SparringInviteScreenBootstrap.notifyExpired();
        assertTrue(
            BongToast.current(System.currentTimeMillis()).text().getString().contains("过期")
        );
    }
}
