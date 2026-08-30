package com.bong.client.social;

import com.bong.client.combat.CombatHudStateStore;
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
        CombatHudStateStore.resetForTests();
    }

    private static SparringInviteScreenBootstrap.CombatState combatStateOf() {
        return SparringInviteScreenBootstrap.combatStateOf();
    }

    private static SparringInviteScreenBootstrap.Decision decide(
        SocialStateStore.SparringInvite invite,
        SparringInviteScreenBootstrap.ScreenKind kind,
        long nowMs,
        SparringInviteScreenBootstrap.CombatState combat
    ) {
        return SparringInviteScreenBootstrap.decide(invite, kind, nowMs, combat);
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
            decide(active, SparringInviteScreenBootstrap.ScreenKind.NONE, 1_000L,
                SparringInviteScreenBootstrap.CombatState.NOT_IN_COMBAT)
        );
        assertEquals(
            SparringInviteScreenBootstrap.Decision.DEFER_NOTIFY,
            decide(
                active,
                SparringInviteScreenBootstrap.ScreenKind.OTHER_SPARRING_INVITE,
                1_000L,
                SparringInviteScreenBootstrap.CombatState.NOT_IN_COMBAT
            ),
            "不同 inviteId 的切磋屏也属于占用态，不能被新邀请强制替换"
        );
        assertEquals(
            SparringInviteScreenBootstrap.Decision.NOOP,
            decide(
                active,
                SparringInviteScreenBootstrap.ScreenKind.MATCHING_SPARRING_INVITE,
                1_000L,
                SparringInviteScreenBootstrap.CombatState.NOT_IN_COMBAT
            )
        );
    }

    @Test
    void activeInviteBlockedByOtherScreenNeverOpensScreen() {
        assertEquals(
            SparringInviteScreenBootstrap.Decision.DEFER_NOTIFY,
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
            SparringInviteScreenBootstrap.Decision.DEFER_NOTIFY,
            SparringInviteScreenBootstrap.decide(second, new SparringInviteScreen(first), 1_000L),
            "真实 screen identity 不同时只能阻塞提示，不能替换当前邀请屏"
        );
    }

    @Test
    void blockedToastIsVisibleAndDeduplicatedPerInviteId() {
        SparringInviteScreenBootstrap.notifyBlocked("invite-1");
        assertFalse(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "首次 blocked 邀请必须显示非阻塞提示"
        );

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
        assertTrue(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "空 inviteId 不得生成无身份提示"
        );

        SparringInviteScreenBootstrap.notifyBlocked("invite-1");
        BongToast.resetForTests();
        SparringInviteScreenBootstrap.resetForTests();
        SparringInviteScreenBootstrap.notifyBlocked("invite-1");
        assertFalse(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "去重状态复位后，同 identity 必须可在新 session 再次提示"
        );
    }

    @Test
    void expiredToastExplainsOutcome() {
        SparringInviteScreenBootstrap.notifyExpired();
        assertTrue(
            BongToast.current(System.currentTimeMillis()).text().getString().contains("过期"),
            "过期提示必须明确说明邀请已过期"
        );
    }

    // ─── R7 P4 combat-aware deferral（server-authoritative combat snapshot）───

    @Test
    void combatStateReadsAuthoritativeCombatHudSnapshot() {
        assertEquals(
            SparringInviteScreenBootstrap.CombatState.UNKNOWN,
            combatStateOf(),
            "无权威 combat snapshot 时必须保持 UNKNOWN 并由策略 fail closed"
        );

        CombatHudStateStore.replaceAuthoritative(
            com.bong.client.combat.CombatHudState.createAuthoritative(
                0.8f,
                0.7f,
                0.9f,
                com.bong.client.combat.DerivedAttrFlags.none(),
                true
            )
        );
        assertEquals(
            SparringInviteScreenBootstrap.CombatState.IN_COMBAT,
            combatStateOf(),
            "server-authoritative combat_hud_state combat_active=true 时必须进入战斗态"
        );

        CombatHudStateStore.replaceAuthoritative(
            com.bong.client.combat.CombatHudState.createAuthoritative(
                0.8f,
                0.7f,
                0.9f,
                com.bong.client.combat.DerivedAttrFlags.none(),
                false
            )
        );
        assertEquals(
            SparringInviteScreenBootstrap.CombatState.NOT_IN_COMBAT,
            combatStateOf(),
            "server-authoritative combat_hud_state combat_active=false 时必须进入脱战态"
        );

        CombatHudStateStore.clear();
        assertEquals(
            SparringInviteScreenBootstrap.CombatState.UNKNOWN,
            combatStateOf(),
            "combat_hud_state 清空后必须回到 UNKNOWN，避免没有权威输入时放行"
        );
    }

    @Test
    void combatFirstObservationDefersAndNotifies() {
        SocialStateStore.SparringInvite active = invite("combat-first", 5_000L);
        SparringInviteScreenBootstrap.resetForTests();

        SparringInviteScreenBootstrap.Decision first =
            decide(active, SparringInviteScreenBootstrap.ScreenKind.NONE, 1_000L,
                SparringInviteScreenBootstrap.CombatState.IN_COMBAT);

        assertEquals(
            SparringInviteScreenBootstrap.Decision.DEFER_NOTIFY,
            first,
            "战斗中首次观察到邀请必须 DEFER_NOTIFY（toast 一次）"
        );
        assertFalse(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "DEFER_NOTIFY 必须产生一次可见提示"
        );
    }

    @Test
    void combatRepeatedObservationDefersSilently() {
        SocialStateStore.SparringInvite active = invite("combat-repeat", 5_000L);
        SparringInviteScreenBootstrap.resetForTests();
        SparringInviteScreenBootstrap.decide(active, SparringInviteScreenBootstrap.ScreenKind.NONE,
            1_000L, SparringInviteScreenBootstrap.CombatState.IN_COMBAT);
        BongToast.resetForTests();

        SparringInviteScreenBootstrap.Decision repeat =
            decide(active, SparringInviteScreenBootstrap.ScreenKind.NONE, 2_000L,
                SparringInviteScreenBootstrap.CombatState.IN_COMBAT);

        assertEquals(
            SparringInviteScreenBootstrap.Decision.DEFER_SILENT,
            repeat,
            "同一 identity 的重复战斗观察必须 DEFER_SILENT"
        );
        assertTrue(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "DEFER_SILENT 不得产生第二次 toast"
        );
    }

    @Test
    void newIdentityRestoresNotificationEligibility() {
        SocialStateStore.SparringInvite firstInvite = invite("combat-first-id", 5_000L);
        SocialStateStore.SparringInvite secondInvite = invite("combat-second-id", 6_000L);
        SparringInviteScreenBootstrap.resetForTests();
        SparringInviteScreenBootstrap.decide(firstInvite, SparringInviteScreenBootstrap.ScreenKind.NONE,
            1_000L, SparringInviteScreenBootstrap.CombatState.IN_COMBAT);
        BongToast.resetForTests();

        SparringInviteScreenBootstrap.Decision newIdentity =
            decide(secondInvite, SparringInviteScreenBootstrap.ScreenKind.NONE, 2_000L,
                SparringInviteScreenBootstrap.CombatState.IN_COMBAT);

        assertEquals(
            SparringInviteScreenBootstrap.Decision.DEFER_NOTIFY,
            newIdentity,
            "新 identity 必须重新取得通知资格"
        );
        assertFalse(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "新 identity 的首次观察必须再次 toast"
        );
    }

    @Test
    void combatNeverOpensScreen() {
        SocialStateStore.SparringInvite active = invite("combat-no-open", 5_000L);
        SparringInviteScreenBootstrap.resetForTests();

        assertEquals(
            SparringInviteScreenBootstrap.Decision.DEFER_NOTIFY,
            decide(active, SparringInviteScreenBootstrap.ScreenKind.NONE, 1_000L,
                SparringInviteScreenBootstrap.CombatState.IN_COMBAT),
            "战斗中即使空屏也不得 OPEN_SCREEN"
        );
    }

    @Test
    void notInCombatEmptyScreenOpensWhenNotExpired() {
        SocialStateStore.SparringInvite active = invite("combat-clear", 5_000L);
        SparringInviteScreenBootstrap.resetForTests();

        assertEquals(
            SparringInviteScreenBootstrap.Decision.OPEN_SCREEN,
            decide(active, SparringInviteScreenBootstrap.ScreenKind.NONE, 1_000L,
                SparringInviteScreenBootstrap.CombatState.NOT_IN_COMBAT),
            "脱战 + 空屏 + 未过期必须 OPEN_SCREEN"
        );
    }

    @Test
    void expiredDeclinesBeforeAnyCombatConsideration() {
        SocialStateStore.SparringInvite expired = invite("combat-expired", 1_000L);
        SparringInviteScreenBootstrap.resetForTests();

        for (SparringInviteScreenBootstrap.CombatState combat : SparringInviteScreenBootstrap.CombatState.values()) {
            SparringInviteScreenBootstrap.Decision decision =
                decide(expired, SparringInviteScreenBootstrap.ScreenKind.NONE, 1_000L, combat);
            assertEquals(
                SparringInviteScreenBootstrap.Decision.DECLINE_EXPIRED,
                decision,
                "过期检查必须先于战斗/屏幕占用判定，combat=" + combat
            );
        }
    }

    @Test
    void combatDoesNotAffectMatchingScreenOrOtherScreens() {
        SocialStateStore.SparringInvite active = invite("combat-matching", 5_000L);
        SparringInviteScreenBootstrap.resetForTests();

        assertEquals(
            SparringInviteScreenBootstrap.Decision.NOOP,
            decide(active, SparringInviteScreenBootstrap.ScreenKind.MATCHING_SPARRING_INVITE, 1_000L,
                SparringInviteScreenBootstrap.CombatState.IN_COMBAT),
            "战斗不改变已匹配邀请屏的保持语义"
        );
        assertEquals(
            SparringInviteScreenBootstrap.Decision.DEFER_NOTIFY,
            decide(active, SparringInviteScreenBootstrap.ScreenKind.OTHER_SPARRING_INVITE, 1_000L,
                SparringInviteScreenBootstrap.CombatState.IN_COMBAT),
            "不同 invite 屏仍按既有占用规则处理"
        );
    }

    @Test
    void missingCombatProducerFailsClosedWithoutOpening() {
        // CombatHudStateStore 从初始 empty() 态模拟 producer 尚未发送任何权威快照。
        SocialStateStore.SparringInvite active = invite("combat-unknown", 5_000L);
        SparringInviteScreenBootstrap.resetForTests();
        CombatHudStateStore.resetForTests();

        SparringInviteScreenBootstrap.Decision decision =
            SparringInviteScreenBootstrap.decide(
                active, SparringInviteScreenBootstrap.ScreenKind.NONE, 1_000L
            );

        assertEquals(SparringInviteScreenBootstrap.CombatState.UNKNOWN, combatStateOf());
        assertEquals(
            SparringInviteScreenBootstrap.Decision.DEFER_NOTIFY,
            decision,
            "缺少 authoritative combat producer 时 P4 fail closed：不得 OPEN_SCREEN"
        );
    }
}
