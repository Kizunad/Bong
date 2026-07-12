package com.bong.client.network;

import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.social.NicheGuardianStore;
import com.bong.client.social.SocialStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class SocialServerDataHandlerTest {
    @AfterEach
    void tearDown() {
        SocialStateStore.resetForTests();
        NicheGuardianStore.resetForTests();
        UnifiedEventStore.resetForTests();
    }

    @Test
    void anonymityPayloadReplacesRemoteIdentitySnapshot() {
        ServerDataDispatch dispatch = handler().handle(parseEnvelope("""
            {"v":1,"type":"social_anonymity","viewer":"char:steve","remotes":[
              {"player_uuid":"offline:NewPlayer1","anonymous":true,"realm_band":"awaken_induce",
               "breath_hint":"气息微弱","renown_tags":[]},
              {"player_uuid":"offline:KnownAlly","anonymous":false,"display_name":"KnownAlly",
               "realm_band":"condense_solidify","breath_hint":"气息在你之上","renown_tags":["kept_pact"]}
            ]}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals("char:steve", SocialStateStore.anonymity().viewer());
        assertEquals(2, SocialStateStore.anonymity().remotesByUuid().size());
        SocialStateStore.SocialRemoteIdentity ally = SocialStateStore.anonymity().remotesByUuid().get("offline:KnownAlly");
        assertNotNull(ally);
        assertFalse(ally.anonymous());
        assertEquals("KnownAlly", ally.displayName());
        assertEquals("kept_pact", ally.renownTags().get(0));
        assertFalse(SocialStateStore.shouldShowRemoteNameTag("", "NewPlayer1"));
        assertTrue(SocialStateStore.shouldShowRemoteNameTag("", "KnownAlly"));
        assertTrue(SocialStateStore.shouldShowRemoteNameTag("offline:KnownAlly", ""));
        assertFalse(SocialStateStore.shouldShowRemoteNameTag("", "Unknown"));
    }

    @Test
    void nameTagPolicyMatchesCharacterScopedOfflineIds() {
        SocialStateStore.replaceAnonymity("char:steve", List.of(
            new SocialStateStore.SocialRemoteIdentity(
                "offline:KnownAlly:char-uuid",
                false,
                "KnownAlly",
                "condense_solidify",
                "气息在你之上",
                List.of()
            )
        ));

        assertTrue(SocialStateStore.shouldShowRemoteNameTag("", "KnownAlly"));
        assertTrue(SocialStateStore.shouldShowRemoteNameTag("offline:KnownAlly:char-uuid", ""));
        assertFalse(SocialStateStore.shouldShowRemoteNameTag("", "Other"));
    }

    @Test
    void exposureRecordsEventAndPublishesHudSignal() {
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route("""
            {"v":1,"type":"social_exposure","actor":"char:steve","kind":"chat",
             "witnesses":["char:new_player_1"],"tick":84000,"zone":"starter_valley"}
            """, 0);

        assertTrue(result.isHandled(), result.logMessage());
        assertEquals(1, SocialStateStore.exposures().size());
        assertEquals("char:steve", SocialStateStore.exposures().get(0).actor());
        assertEquals("char:new_player_1", SocialStateStore.exposures().get(0).witnesses().get(0));
        assertEquals(1, UnifiedEventStore.stream().size());
        UnifiedEvent event = UnifiedEventStore.stream().snapshot().get(0);
        assertEquals(UnifiedEvent.Channel.SOCIAL, event.channel());
        assertTrue(event.text().contains("身份暴露"));
    }

    @Test
    void exposureKeepsNameTagHiddenUntilAuthoritativeAnonymityRefreshArrives() {
        SocialStateStore.replaceAnonymity("char:witness", List.of(
            new SocialStateStore.SocialRemoteIdentity(
                "char:steve",
                true,
                "",
                "",
                "无名修士",
                List.of()
            )
        ));

        ServerDataRouter.RouteResult exposure = ServerDataRouter.createDefault().route("""
            {"v":1,"type":"social_exposure","actor":"char:steve","kind":"chat",
             "witnesses":["char:witness"],"tick":84000,"zone":"starter_valley"}
            """, 0);
        boolean exposureHandled = exposure.isHandled();
        assertTrue(
            exposureHandled,
            "expected social_exposure to be handled because it updates event history, actual "
                + exposureHandled + "; log=" + exposure.logMessage()
        );
        boolean visibleAfterExposure =
            SocialStateStore.shouldShowRemoteNameTag("char:steve", "Steve");
        assertFalse(
            visibleAfterExposure,
            "expected name tag hidden because social_exposure is history-only until social_anonymity, actual "
                + visibleAfterExposure
        );

        ServerDataRouter.RouteResult anonymity = ServerDataRouter.createDefault().route("""
            {"v":1,"type":"social_anonymity","viewer":"char:witness","remotes":[
              {"player_uuid":"char:steve","anonymous":false,"display_name":"char:steve",
               "realm_band":null,"breath_hint":"无名修士","renown_tags":[]}
            ]}
            """, 0);
        boolean anonymityHandled = anonymity.isHandled();
        assertTrue(
            anonymityHandled,
            "expected social_anonymity refresh to be handled because it is authoritative, actual "
                + anonymityHandled + "; log=" + anonymity.logMessage()
        );
        boolean visibleAfterAnonymity =
            SocialStateStore.shouldShowRemoteNameTag("char:steve", "Steve");
        assertTrue(
            visibleAfterAnonymity,
            "expected name tag visible because authoritative social_anonymity removed anonymity, actual "
                + visibleAfterAnonymity
        );
    }

    @Test
    void pactFeudRenownAndSparringUpdateStores() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        assertTrue(router.route("""
            {"v":1,"type":"social_pact","left":"char:steve","right":"char:new_player_1",
             "terms":"同行守望","tick":84200,"broken":false}
            """, 0).isHandled());
        assertTrue(router.route("""
            {"v":1,"type":"social_feud","left":"char:steve","right":"char:bandit_01",
             "tick":84300,"place":"blood_valley"}
            """, 0).isHandled());
        assertTrue(router.route("""
            {"v":1,"type":"social_renown_delta","char_id":"char:steve","fame_delta":1,
             "notoriety_delta":0,"tags_added":[{"tag":"kept_pact","weight":1,
             "last_seen_tick":84400,"permanent":false}],"tick":84400,"reason":"pact_kept"}
            """, 0).isHandled());
        assertTrue(router.route("""
            {"v":1,"type":"sparring_invite","invite_id":"sparring:84000:steve:new_player_1",
             "initiator":"char:steve","target":"char:new_player_1","realm_band":"awaken_induce",
             "breath_hint":"气息相近","terms":"点到为止","expires_at_ms":1712346000000}
            """, 0).isHandled());

        assertEquals(2, SocialStateStore.relationships().size());
        assertEquals("feud", SocialStateStore.relationships().get(0).kind());
        assertEquals("pact", SocialStateStore.relationships().get(1).kind());
        assertEquals(1, SocialStateStore.renownDeltas().size());
        assertEquals("kept_pact", SocialStateStore.renownDeltas().get(0).tagsAdded().get(0).tag());
        assertNotNull(SocialStateStore.sparringInvite());
        assertEquals("char:new_player_1", SocialStateStore.sparringInvite().target());
        assertEquals(4, UnifiedEventStore.stream().size());
    }

    @Test
    void tradeOfferUpdatesStoreAndPublishesHudSignal() {
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route("""
            {"v":1,"type":"trade_offer","offer_id":"trade:char:steve:char:new:1001:42",
             "initiator":"char:steve","target":"char:new",
             "offered_item":{"instance_id":1001,"item_id":"spirit_grass","display_name":"Spirit Grass","stack_count":1},
             "requested_items":[{"instance_id":2002,"item_id":"bone_coin","display_name":"Bone Coin","stack_count":3}],
             "expires_at_ms":1712346000000}
            """, 0);

        assertTrue(result.isHandled(), result.logMessage());
        assertNotNull(SocialStateStore.tradeOffer());
        assertEquals("trade:char:steve:char:new:1001:42", SocialStateStore.tradeOffer().offerId());
        assertEquals("Spirit Grass", SocialStateStore.tradeOffer().offeredItem().displayName());
        assertEquals(2002L, SocialStateStore.tradeOffer().requestedItems().get(0).instanceId());
        assertEquals(1, UnifiedEventStore.stream().size());
    }

    @Test
    void duplicateSparringInvitePayloadIsIgnoredWithoutRepublishing() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        String payload = sparringInvitePayload("sparring:duplicate", 5_000L);

        ServerDataRouter.RouteResult first = router.route(payload, 0);
        assertTrue(first.isHandled(), first.logMessage());
        assertEquals(
            SocialStateStore.SparringInviteUpdate.DUPLICATE,
            SocialStateStore.enqueueSparringInvite(sparringInvite("sparring:duplicate", 5_000L)),
            "同 identity 的直接 store 入队必须明确返回 DUPLICATE"
        );
        ServerDataRouter.RouteResult duplicate = router.route(payload, 0);

        assertTrue(duplicate.isNoOp(), duplicate.logMessage());
        assertEquals("sparring:duplicate", SocialStateStore.sparringInvite().inviteId());
        assertEquals(1, UnifiedEventStore.stream().size(), "重复邀请不能重复发布 HUD 事件");
    }

    @Test
    void lateOlderSparringInviteCannotReplaceNewerInvite() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        ServerDataRouter.RouteResult current = router.route(sparringInvitePayload("sparring:0002", 6_000L), 0);
        assertTrue(current.isHandled(), current.logMessage());
        ServerDataRouter.RouteResult late = router.route(sparringInvitePayload("sparring:0001", 5_000L), 0);

        assertTrue(late.isNoOp(), late.logMessage());
        assertEquals("sparring:0002", SocialStateStore.sparringInvite().inviteId());
        assertEquals(1, UnifiedEventStore.stream().size(), "迟到旧邀请不能覆盖新邀请或重复发布 HUD 事件");
    }

    @Test
    void lateOlderSparringInviteWithSameExpiryIsIgnoredByUuidV7Order() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        ServerDataRouter.RouteResult current = router.route(sparringInvitePayload("sparring:0002", 6_000L), 0);
        assertTrue(current.isHandled(), current.logMessage());
        ServerDataRouter.RouteResult late = router.route(sparringInvitePayload("sparring:0001", 6_000L), 0);

        assertTrue(late.isNoOp(), late.logMessage());
        assertEquals("sparring:0002", SocialStateStore.sparringInvite().inviteId());
    }

    @Test
    void settledInviteReplayCannotDisplaceQueuedInvite() {
        SocialStateStore.SparringInvite first = sparringInvite("sparring:first", 5_000L);
        SocialStateStore.SparringInvite second = sparringInvite("sparring:second", 6_000L);

        assertEquals(SocialStateStore.SparringInviteUpdate.ACCEPTED, SocialStateStore.enqueueSparringInvite(first));
        assertEquals(SocialStateStore.SparringInviteUpdate.ACCEPTED, SocialStateStore.enqueueSparringInvite(second));
        assertEquals("sparring:first", SocialStateStore.sparringInvite().inviteId());

        SocialStateStore.clearSparringInvite(first.inviteId());
        assertEquals("sparring:second", SocialStateStore.sparringInvite().inviteId());
        assertEquals(SocialStateStore.SparringInviteUpdate.SETTLED, SocialStateStore.enqueueSparringInvite(first));
        assertEquals("sparring:second", SocialStateStore.sparringInvite().inviteId());
    }

    @Test
    void clearAndConcurrentEnqueueNeverLoseNewInvite() throws InterruptedException {
        for (int iteration = 0; iteration < 32; iteration++) {
            SocialStateStore.resetForTests();
            SocialStateStore.SparringInvite first = sparringInvite("sparring:first:" + iteration, 5_000L);
            SocialStateStore.SparringInvite second = sparringInvite("sparring:second:" + iteration, 6_000L);
            SocialStateStore.enqueueSparringInvite(first);
            CountDownLatch start = new CountDownLatch(1);
            AtomicReference<Throwable> failure = new AtomicReference<>();
            Thread clearThread = new Thread(() -> runAfter(start, failure, () ->
                SocialStateStore.clearSparringInvite(first.inviteId())
            ));
            Thread enqueueThread = new Thread(() -> runAfter(start, failure, () ->
                SocialStateStore.enqueueSparringInvite(second)
            ));

            clearThread.start();
            enqueueThread.start();
            start.countDown();
            clearThread.join();
            enqueueThread.join();

            if (failure.get() != null) {
                throw new AssertionError("并发邀请 store 操作异常，iteration=" + iteration, failure.get());
            }
            assertEquals(
                second.inviteId(),
                SocialStateStore.sparringInvite().inviteId(),
                "clear A 与 enqueue B 任意交错都不能吞掉 B，iteration=" + iteration
            );
        }
    }

    @Test
    void invalidInviteAndUnknownClearLeaveStoreEmpty() {
        assertEquals(SocialStateStore.SparringInviteUpdate.INVALID, SocialStateStore.enqueueSparringInvite(null));
        assertEquals(
            SocialStateStore.SparringInviteUpdate.INVALID,
            SocialStateStore.enqueueSparringInvite(sparringInvite("   ", 5_000L))
        );

        SocialStateStore.clearSparringInvite("unknown");

        assertNull(SocialStateStore.sparringInvite());
        assertEquals(
            SocialStateStore.SparringInviteUpdate.ACCEPTED,
            SocialStateStore.enqueueSparringInvite(sparringInvite("unknown", 5_000L)),
            "清理不存在的 ID 不能制造 tombstone"
        );
    }

    @Test
    void blankInviteIdentityCannotClearCurrentInvite() {
        SocialStateStore.SparringInvite current = sparringInvite("sparring:current", 5_000L);
        assertEquals(SocialStateStore.SparringInviteUpdate.ACCEPTED, SocialStateStore.enqueueSparringInvite(current));

        SocialStateStore.clearSparringInvite("   ");

        assertNotNull(SocialStateStore.sparringInvite(), "空 identity 不能退化为清理当前邀请，否则迟到 UI 会误清后继状态");
        assertEquals(current.inviteId(), SocialStateStore.sparringInvite().inviteId());
    }

    @Test
    void pendingSparringInviteQueueHasStableCapacityBoundary() {
        for (int index = 0; index < 64; index++) {
            assertEquals(
                SocialStateStore.SparringInviteUpdate.ACCEPTED,
                SocialStateStore.enqueueSparringInvite(sparringInvite("sparring:" + index, 5_000L + index)),
                "容量边界内的邀请必须保留，index=" + index
            );
        }
        SocialStateStore.SparringInvite overflow = sparringInvite("sparring:overflow", 6_000L);

        assertEquals(
            SocialStateStore.SparringInviteUpdate.CAPACITY,
            SocialStateStore.enqueueSparringInvite(overflow),
            "第 65 份 pending 邀请必须被拒绝，避免网络输入导致客户端队列无界增长"
        );
        assertEquals("sparring:0", SocialStateStore.sparringInvite().inviteId(), "容量拒绝不能扰动当前邀请 identity");

        SocialStateStore.clearSparringInvite("sparring:0");
        assertEquals(
            SocialStateStore.SparringInviteUpdate.ACCEPTED,
            SocialStateStore.enqueueSparringInvite(overflow),
            "释放容量后，先前被拒邀请必须可重试；CAPACITY 不能污染版本高水位或 tombstone"
        );
    }

    @Test
    void nicheIntrusionAndGuardianEventsUpdateDefenseStore() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        assertTrue(router.route("""
            {"v":1,"type":"niche_intrusion","niche_pos":[1,64,2],"intruder_id":"char:raider",
             "items_taken":[42,43],"taint_delta":0.2}
            """, 0).isHandled());
        assertTrue(router.route("""
            {"v":1,"type":"niche_guardian_fatigue","guardian_kind":"puppet","charges_remaining":4}
            """, 0).isHandled());
        assertTrue(router.route("""
            {"v":1,"type":"niche_guardian_broken","guardian_kind":"puppet","intruder_id":"char:raider"}
            """, 0).isHandled());

        assertEquals(2, NicheGuardianStore.intrusionAlerts().size());
        assertEquals("char:raider", NicheGuardianStore.intrusionAlerts().get(0).intruderId());
        assertTrue(NicheGuardianStore.guardianStatuses().get("puppet").broken());
        assertEquals(3, UnifiedEventStore.stream().size());
    }

    @Test
    void invalidSocialPayloadBecomesSafeNoOp() {
        ServerDataDispatch dispatch = handler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"social_exposure\",\"actor\":\"char:steve\",\"kind\":\"unknown\",\"witnesses\":[],\"tick\":1}"
        ));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("social_exposure"));
        assertEquals(0, SocialStateStore.exposures().size());
    }

    private static SocialServerDataHandler handler() {
        return new SocialServerDataHandler();
    }

    private static String sparringInvitePayload(String inviteId, long expiresAtMs) {
        return "{\"v\":1,\"type\":\"sparring_invite\",\"invite_id\":\"" + inviteId
            + "\",\"initiator\":\"char:a\",\"target\":\"char:b\",\"realm_band\":\"凝脉\","
            + "\"breath_hint\":\"气息相试\",\"terms\":\"点到为止\",\"expires_at_ms\":"
            + expiresAtMs + "}";
    }

    private static SocialStateStore.SparringInvite sparringInvite(String inviteId, long expiresAtMs) {
        return new SocialStateStore.SparringInvite(
            inviteId,
            "char:a",
            "char:b",
            "凝脉",
            "气息相试",
            "点到为止",
            expiresAtMs
        );
    }

    private static void runAfter(CountDownLatch start, AtomicReference<Throwable> failure, Runnable action) {
        try {
            start.await();
            action.run();
        } catch (Throwable throwable) {
            failure.compareAndSet(null, throwable);
        }
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json,
            json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
