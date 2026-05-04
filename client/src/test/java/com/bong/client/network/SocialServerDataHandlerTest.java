package com.bong.client.network;

import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.social.NicheGuardianStore;
import com.bong.client.social.SocialStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
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
        ServerDataDispatch dispatch = handler().handle(parseEnvelope("""
            {"v":1,"type":"trade_offer","offer_id":"trade:char:steve:char:new:1001:42",
             "initiator":"char:steve","target":"char:new",
             "offered_item":{"instance_id":1001,"item_id":"spirit_grass","display_name":"Spirit Grass","stack_count":1},
             "requested_items":[{"instance_id":2002,"item_id":"bone_coin","display_name":"Bone Coin","stack_count":3}],
             "expires_at_ms":1712346000000}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertNotNull(SocialStateStore.tradeOffer());
        assertEquals("trade:char:steve:char:new:1001:42", SocialStateStore.tradeOffer().offerId());
        assertEquals("Spirit Grass", SocialStateStore.tradeOffer().offeredItem().displayName());
        assertEquals(2002L, SocialStateStore.tradeOffer().requestedItems().get(0).instanceId());
        assertEquals(1, UnifiedEventStore.stream().size());
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

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json,
            json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
