package com.bong.client.social;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.network.ClientRequestSender;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class SparringInviteScreenTest {
    private final List<String> sentPayloads = new ArrayList<>();

    @AfterEach
    void reset() {
        ClientRequestSender.resetBackendForTests();
        SocialStateStore.resetForTests();
        CombatHudStateStore.resetForTests();
    }

    @Test
    void describeShowsAnonymousTermsAndCountdown() {
        SocialStateStore.SparringInvite invite = invite("sparring:1:a:b", 1234L);

        SparringInviteScreen.RenderContent content = SparringInviteScreen.describe(invite, 9_000L);

        assertTrue(content.lines().contains("发起者气息: 气息相试"), "描述必须展示匿名气息提示");
        assertTrue(content.lines().contains("境界段: condense_solidify"), "描述必须展示境界段");
        assertTrue(content.lines().contains("条款: 点到为止"), "描述必须展示切磋条款");
        assertTrue(content.lines().contains("倒计时: 9s"), "描述必须展示剩余秒数");
        assertTrue(
            content.lines().contains("失败方: 5min 谦抑, 真元回复 -30%"),
            "描述必须展示失败代价"
        );
    }

    @Test
    void closeSettlesExactInviteAndPromotesQueuedInvite() {
        SocialStateStore.SparringInvite first = invite("sparring:first", 5_000L);
        SocialStateStore.SparringInvite second = invite("sparring:second", 6_000L);
        SocialStateStore.enqueueSparringInvite(first);
        SocialStateStore.enqueueSparringInvite(second);
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, StandardCharsets.UTF_8))
        );
        // 排队晋级测试显式注入 server 权威脱战帧，避免把缺失 producer 当成可开屏。
        CombatHudStateStore.replaceAuthoritative(
            CombatHudState.createAuthoritative(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none(), false)
        );

        new SparringInviteScreen(first).close();

        assertEquals(1, sentPayloads.size(), "关闭邀请屏只发送一次拒绝响应");
        assertTrue(
            sentPayloads.get(0).contains("\"invite_id\":\"sparring:first\""),
            "关闭响应必须携带当前 invite identity，实际 payload=" + sentPayloads.get(0)
        );
        assertTrue(
            sentPayloads.get(0).contains("\"accepted\":false"),
            "关闭邀请屏必须发送拒绝语义，实际 payload=" + sentPayloads.get(0)
        );
        assertEquals("sparring:second", SocialStateStore.sparringInvite().inviteId());
        assertEquals(
            SparringInviteScreenBootstrap.Decision.OPEN_SCREEN,
            SparringInviteScreenBootstrap.decide(
                SocialStateStore.sparringInvite(),
                SparringInviteScreenBootstrap.ScreenKind.NONE,
                1_000L
            ),
            "关闭 A 后，无其他屏时应展示排队的 B"
        );
    }

    @Test
    void lateCloseOfOldScreenCannotClearReplacement() {
        SocialStateStore.SparringInvite first = invite("sparring:first", 5_000L);
        SocialStateStore.SparringInvite second = invite("sparring:second", 6_000L);
        SocialStateStore.enqueueSparringInvite(first);
        SocialStateStore.enqueueSparringInvite(second);
        SocialStateStore.clearSparringInvite(first.inviteId());
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, StandardCharsets.UTF_8))
        );

        new SparringInviteScreen(first).close();

        assertTrue(sentPayloads.isEmpty(), "已结算旧 screen 的迟到 close 不得重复发送拒绝响应");
        assertEquals(
            "sparring:second",
            SocialStateStore.sparringInvite().inviteId(),
            "旧 screen 的迟到 close 只能清自己的 identity，不能误清后继邀请"
        );
    }

    @Test
    void duplicateScreensForSameInviteSendOnlyOneResponse() {
        SocialStateStore.SparringInvite invite = invite("sparring:duplicate-screen", 5_000L);
        SocialStateStore.enqueueSparringInvite(invite);
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, StandardCharsets.UTF_8))
        );

        new SparringInviteScreen(invite).close();
        new SparringInviteScreen(invite).close();

        assertEquals(1, sentPayloads.size(), "同一 invite identity 即使残留两个 screen，也只能结算一次");
    }

    @Test
    void acceptingInviteAtomicallySettlesQueuedInvites() {
        SocialStateStore.SparringInvite first = invite("sparring:first", 5_000L);
        SocialStateStore.SparringInvite second = invite("sparring:second", 6_000L);
        SocialStateStore.SparringInvite third = invite("sparring:third", 7_000L);
        SocialStateStore.enqueueSparringInvite(first);
        SocialStateStore.enqueueSparringInvite(second);
        SocialStateStore.enqueueSparringInvite(third);
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, StandardCharsets.UTF_8))
        );

        new SparringInviteScreen(first).acceptForTests();

        assertNull(SocialStateStore.sparringInvite(), "接受一场切磋后不得继续弹出排队邀请");
        assertEquals(1, sentPayloads.size(), "接受当前邀请只发送自身响应，其他 pending 由本地 tombstone 与服务端 TTL 清理");
        assertTrue(
            sentPayloads.get(0).contains("\"invite_id\":\"sparring:first\""),
            "接受响应必须携带当前 invite identity，实际 payload=" + sentPayloads.get(0)
        );
        assertTrue(
            sentPayloads.get(0).contains("\"accepted\":true"),
            "接受入口必须发送应战语义，实际 payload=" + sentPayloads.get(0)
        );
        assertEquals(
            SocialStateStore.SparringInviteUpdate.SETTLED,
            SocialStateStore.enqueueSparringInvite(second),
            "接受当前邀请时被拒绝的 pending identity 也必须进入 tombstone，防止迟到重放"
        );
    }

    private static SocialStateStore.SparringInvite invite(String inviteId, long expiresAtMs) {
        return new SocialStateStore.SparringInvite(
            inviteId,
            "char:a",
            "char:b",
            "condense_solidify",
            "气息相试",
            "点到为止",
            expiresAtMs
        );
    }
}
