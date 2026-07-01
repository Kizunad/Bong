package com.bong.client.social;

import com.bong.client.hud.BongToast;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * F4 fix — 交易邀请被其他 GUI 挡住 / 静默过期时，玩家此前完全零感知（{@code handleIncomingOffer}
 * 直接 silent return 或 auto-decline，无任何提示）。覆盖新引入的 {@link TradeOfferScreenBootstrap#decide}
 * 纯决策函数（脱离 MinecraftClient/Screen 依赖，饱和覆盖全部 (offer × screenKind) 组合）
 * 以及 {@code notifyBlocked}/{@code notifyExpired} 两个 toast 副作用入口。
 */
class TradeOfferScreenBootstrapTest {
    @AfterEach
    void reset() {
        TradeOfferScreenBootstrap.resetForTests();
        BongToast.resetForTests();
    }

    private static SocialStateStore.TradeOffer offer(String id, long expiresAtMs) {
        return new SocialStateStore.TradeOffer(
            id,
            "char:initiator",
            "char:target",
            new SocialStateStore.TradeItemSummary(1L, "iron_sword", "铁剑", 1),
            List.of(),
            expiresAtMs
        );
    }

    // ── decide(): offer == null ─────────────────────────────────────────────

    @Test
    void noOfferAndNoScreenIsNoop() {
        assertEquals(
            TradeOfferScreenBootstrap.Decision.NOOP,
            TradeOfferScreenBootstrap.decide(null, TradeOfferScreenBootstrap.ScreenKind.NONE, 1_000L),
            "expected NOOP when there is no offer and no screen open, actual decision differs"
        );
    }

    @Test
    void noOfferAndOtherScreenIsNoop() {
        assertEquals(
            TradeOfferScreenBootstrap.Decision.NOOP,
            TradeOfferScreenBootstrap.decide(null, TradeOfferScreenBootstrap.ScreenKind.OTHER, 1_000L),
            "expected NOOP when there is no offer and an unrelated screen is open, actual decision differs"
        );
    }

    @Test
    void noOfferButMatchingTradeScreenOpenClosesIt() {
        assertEquals(
            TradeOfferScreenBootstrap.Decision.CLOSE_SCREEN,
            TradeOfferScreenBootstrap.decide(null, TradeOfferScreenBootstrap.ScreenKind.MATCHING_TRADE_OFFER, 1_000L),
            "expected CLOSE_SCREEN because a stale TradeOfferScreen must not linger once the offer is gone"
        );
    }

    @Test
    void noOfferButStaleTradeScreenOpenClosesIt() {
        assertEquals(
            TradeOfferScreenBootstrap.Decision.CLOSE_SCREEN,
            TradeOfferScreenBootstrap.decide(null, TradeOfferScreenBootstrap.ScreenKind.OTHER_TRADE_OFFER, 1_000L),
            "expected CLOSE_SCREEN for a stale TradeOfferScreen too (any TradeOfferScreen kind), actual decision differs"
        );
    }

    // ── decide(): expiry — must decline regardless of screen kind ──────────

    @Test
    void expiredOfferDeclinesRegardlessOfScreenKind() {
        SocialStateStore.TradeOffer expired = offer("t-expired", 500L);
        for (TradeOfferScreenBootstrap.ScreenKind kind : TradeOfferScreenBootstrap.ScreenKind.values()) {
            assertEquals(
                TradeOfferScreenBootstrap.Decision.DECLINE_EXPIRED,
                TradeOfferScreenBootstrap.decide(expired, kind, 1_000L),
                "expected DECLINE_EXPIRED for expiresAtMs=500 <= now=1000 regardless of screenKind=" + kind
                    + ", because expiry must always auto-decline even while a GUI is open"
            );
        }
    }

    @Test
    void expiryBoundaryEqualToNowCountsAsExpired() {
        SocialStateStore.TradeOffer boundary = offer("t-boundary", 1_000L);
        assertEquals(
            TradeOfferScreenBootstrap.Decision.DECLINE_EXPIRED,
            TradeOfferScreenBootstrap.decide(boundary, TradeOfferScreenBootstrap.ScreenKind.NONE, 1_000L),
            "expected DECLINE_EXPIRED when expiresAtMs == nowMs (inclusive boundary, matches offer.expiresAtMs() <= now)"
        );
    }

    @Test
    void justBeforeExpiryIsNotYetExpired() {
        SocialStateStore.TradeOffer notYet = offer("t-not-yet", 1_001L);
        assertNotEquals(
            TradeOfferScreenBootstrap.Decision.DECLINE_EXPIRED,
            TradeOfferScreenBootstrap.decide(notYet, TradeOfferScreenBootstrap.ScreenKind.NONE, 1_000L),
            "expected non-expiry decision when expiresAtMs=1001 > now=1000, actual was DECLINE_EXPIRED"
        );
    }

    // ── decide(): active offer, screen dispatch ─────────────────────────────

    @Test
    void activeOfferWithNoScreenOpensScreen() {
        SocialStateStore.TradeOffer active = offer("t-active", 5_000L);
        assertEquals(
            TradeOfferScreenBootstrap.Decision.OPEN_SCREEN,
            TradeOfferScreenBootstrap.decide(active, TradeOfferScreenBootstrap.ScreenKind.NONE, 1_000L)
        );
    }

    @Test
    void activeOfferWithStaleTradeScreenReopensFreshScreen() {
        SocialStateStore.TradeOffer active = offer("t-active", 5_000L);
        assertEquals(
            TradeOfferScreenBootstrap.Decision.OPEN_SCREEN,
            TradeOfferScreenBootstrap.decide(active, TradeOfferScreenBootstrap.ScreenKind.OTHER_TRADE_OFFER, 1_000L),
            "expected OPEN_SCREEN to replace a stale TradeOfferScreen showing a different offerId"
        );
    }

    @Test
    void activeOfferWithMatchingTradeScreenIsNoop() {
        SocialStateStore.TradeOffer active = offer("t-active", 5_000L);
        assertEquals(
            TradeOfferScreenBootstrap.Decision.NOOP,
            TradeOfferScreenBootstrap.decide(active, TradeOfferScreenBootstrap.ScreenKind.MATCHING_TRADE_OFFER, 1_000L),
            "expected NOOP — the TradeOfferScreen already showing this exact offer must not be touched or toasted"
        );
    }

    @Test
    void activeOfferBlockedByOtherScreenTriggersBlockedToastDecision() {
        SocialStateStore.TradeOffer active = offer("t-active", 5_000L);
        assertEquals(
            TradeOfferScreenBootstrap.Decision.BLOCKED_TOAST,
            TradeOfferScreenBootstrap.decide(active, TradeOfferScreenBootstrap.ScreenKind.OTHER, 1_000L),
            "expected BLOCKED_TOAST — this is the F4 bug: previously this branch silently returned with zero player feedback"
        );
    }

    // ── notifyBlocked(): toast side effect + dedup ──────────────────────────

    @Test
    void notifyBlockedShowsNonEmptyToast() {
        TradeOfferScreenBootstrap.notifyBlocked("offer-1");

        assertFalse(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "expected a non-empty BongToast after notifyBlocked(), actual: toast is empty (player still gets zero feedback)"
        );
    }

    @Test
    void notifyBlockedDedupsSameOfferIdAcrossCalls() {
        TradeOfferScreenBootstrap.notifyBlocked("offer-1");
        BongToast.resetForTests();

        TradeOfferScreenBootstrap.notifyBlocked("offer-1");

        assertTrue(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "expected the second notifyBlocked() call for the SAME offer id to be deduped and show no new toast "
                + "(otherwise every tick while blocked would re-spam the toast), actual: a new toast was shown"
        );
    }

    @Test
    void notifyBlockedRetriggersOnDifferentOfferId() {
        TradeOfferScreenBootstrap.notifyBlocked("offer-1");
        BongToast.resetForTests();

        TradeOfferScreenBootstrap.notifyBlocked("offer-2");

        assertFalse(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "expected a NEW offer id to re-trigger the blocked toast even though a different offer was already notified"
        );
    }

    @Test
    void notifyBlockedIgnoresBlankOfferId() {
        TradeOfferScreenBootstrap.notifyBlocked("   ");

        assertTrue(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "expected notifyBlocked() to no-op on a blank offer id (defensive), actual: toast was shown"
        );
    }

    @Test
    void resetForTestsClearsDedupStateAllowingReToast() {
        TradeOfferScreenBootstrap.notifyBlocked("offer-1");
        BongToast.resetForTests();
        TradeOfferScreenBootstrap.resetForTests();

        TradeOfferScreenBootstrap.notifyBlocked("offer-1");

        assertFalse(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "expected resetForTests() to clear the dedup marker so the same offer id can toast again, actual: still deduped"
        );
    }

    // ── notifyExpired(): toast side effect ──────────────────────────────────

    @Test
    void notifyExpiredShowsExpiryToastText() {
        TradeOfferScreenBootstrap.notifyExpired();

        assertTrue(
            BongToast.current(System.currentTimeMillis()).text().getString().contains("过期"),
            "expected the expiry toast text to mention 过期 so the player understands what happened, actual: "
                + BongToast.current(System.currentTimeMillis()).text().getString()
        );
    }
}
