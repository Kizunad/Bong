package com.bong.client.npc;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NpcMetadataHandlerTest {
    @AfterEach
    void clearStore() {
        NpcMetadataStore.clearAll();
    }

    // ── basic handle ────────────────────────────────────────────────────────

    @Test
    void storesValidNpcMetadataPayload() {
        String payload = """
            {"type":"npc_metadata","v":1,"entity_id":42,"archetype":"rogue","realm":"凝脉",\
            "faction_name":"中立盟","faction_rank":"客卿","reputation_to_player":55,\
            "display_name":"散修·凝脉","age_band":"正值壮年","greeting_text":"道友，可有灵草出让？",\
            "qi_hint":"真元流转平稳"}
            """.trim();

        assertTrue(NpcMetadataHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length));

        NpcMetadata metadata = NpcMetadataStore.get(42);
        assertNotNull(metadata, "metadata should be stored for entity_id 42");
        assertEquals("rogue", metadata.archetype());
        assertEquals("散修·凝脉", metadata.displayName());
        assertEquals("道友，可有灵草出让？", metadata.greetingText());
        // No trade_offers in payload -> tradeCandidate() is false (data-driven)
        assertFalse(metadata.tradeCandidate(),
            "tradeCandidate should be false when no trade_offers are present");
    }

    @Test
    void storesAnonymousDisciplePayloadFromServerContract() {
        String payload = """
            {"type":"npc_metadata","v":1,"entity_id":43,"archetype":"disciple","realm":"凝脉",\
            "reputation_to_player":80,"display_name":"残宗余孽·凝脉",\
            "age_band":"正值壮年","greeting_text":"道友，可有灵草出让？"}
            """.trim();

        assertTrue(NpcMetadataHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length));

        NpcMetadata metadata = NpcMetadataStore.get(43);
        assertNotNull(metadata, "metadata should be stored for anonymous disciple payload");
        assertEquals("disciple", metadata.archetype());
        assertEquals("残宗余孽·凝脉", metadata.displayName());
        assertEquals(null, metadata.factionName(), "server anonymity contract should omit faction_name");
        assertEquals(null, metadata.factionRank(), "server anonymity contract should omit faction_rank");
    }

    @Test
    void rejectsOversizeNpcMetadataPayload() {
        String payload = "{\"type\":\"npc_metadata\",\"v\":1,\"entity_id\":42}";

        assertFalse(NpcMetadataHandler.handle(payload, ServerDataEnvelope.MAX_PAYLOAD_BYTES + 1));
        assertEquals(0, NpcMetadataStore.snapshot().size());
    }

    @Test
    void rejectsNegativeEntityId() {
        String payload = """
            {"type":"npc_metadata","v":1,"entity_id":-1,"archetype":"rogue","realm":"凝脉",\
            "reputation_to_player":0,"display_name":"散修·凝脉"}
            """.trim();

        assertFalse(NpcMetadataHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length));
        assertEquals(0, NpcMetadataStore.snapshot().size());
    }

    @Test
    void rejectsWrongVersion() {
        String payload = """
            {"type":"npc_metadata","v":99,"entity_id":1,"archetype":"rogue"}
            """.trim();

        assertFalse(NpcMetadataHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "version != 1 should be rejected");
    }

    @Test
    void rejectsWrongType() {
        String payload = """
            {"type":"other_packet","v":1,"entity_id":1,"archetype":"rogue"}
            """.trim();

        assertFalse(NpcMetadataHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length),
            "type != npc_metadata should be rejected");
    }

    @Test
    void rejectsNullPayload() {
        assertFalse(NpcMetadataHandler.handle(null, 10),
            "null payload should be rejected");
    }

    @Test
    void rejectsMalformedJson() {
        assertFalse(NpcMetadataHandler.handle("{not valid json!!!", 20),
            "malformed JSON should be rejected");
    }

    // ── equipment parsing ───────────────────────────────────────────────────

    @Test
    void parsesEquipmentFromCompleteJson() {
        String payload = """
            {"type":"npc_metadata","v":1,"entity_id":10,"archetype":"rogue","realm":"凝脉",\
            "reputation_to_player":0,"display_name":"测试",\
            "equipment":{"main_hand":{"template_id":"iron_sword","display_name":"铁剑",\
            "quality_tier":0,"durability_ratio":0.85},\
            "chest":{"template_id":"bone_chestplate","display_name":"骨甲",\
            "quality_tier":1,"durability_ratio":0.7}}}
            """.trim();

        assertTrue(NpcMetadataHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length));
        NpcMetadata metadata = NpcMetadataStore.get(10);
        assertNotNull(metadata);
        assertEquals(2, metadata.equipment().size(), "expected 2 equipment slots");
        NpcEquipSlotData main = metadata.equipment().get("main_hand");
        assertNotNull(main, "main_hand should exist");
        assertEquals("铁剑", main.displayName());
        assertEquals(0, main.qualityTier());
        assertTrue(Math.abs(main.durabilityRatio() - 0.85) < 0.01,
            "expected durability_ratio ~0.85 but got " + main.durabilityRatio());
        NpcEquipSlotData chest = metadata.equipment().get("chest");
        assertNotNull(chest, "chest should exist");
        assertEquals("骨甲", chest.displayName());
        assertEquals(1, chest.qualityTier());
    }

    @Test
    void parsesEquipmentMissingFieldFallsBackEmpty() {
        String json = """
            {"type":"npc_metadata","v":1,"entity_id":11,"archetype":"beast","realm":"醒灵",\
            "reputation_to_player":0,"display_name":"异兽"}
            """.trim();

        assertTrue(NpcMetadataHandler.handle(json, json.getBytes(StandardCharsets.UTF_8).length));
        NpcMetadata metadata = NpcMetadataStore.get(11);
        assertNotNull(metadata);
        assertTrue(metadata.equipment().isEmpty(),
            "missing equipment field should fallback to empty map");
    }

    @Test
    void parsesEquipmentNullFieldFallsBackEmpty() {
        String json = """
            {"type":"npc_metadata","v":1,"entity_id":12,"archetype":"beast","realm":"醒灵",\
            "reputation_to_player":0,"display_name":"异兽","equipment":null}
            """.trim();

        assertTrue(NpcMetadataHandler.handle(json, json.getBytes(StandardCharsets.UTF_8).length));
        NpcMetadata metadata = NpcMetadataStore.get(12);
        assertNotNull(metadata);
        assertTrue(metadata.equipment().isEmpty(),
            "null equipment field should fallback to empty map");
    }

    @Test
    void parseEquipmentSkipsNullSlotValues() {
        JsonObject root = JsonParser.parseString(
            "{\"equipment\":{\"main_hand\":null,\"chest\":{\"template_id\":\"a\",\"display_name\":\"b\",\"quality_tier\":0,\"durability_ratio\":1.0}}}"
        ).getAsJsonObject();

        Map<String, NpcEquipSlotData> result = NpcMetadataHandler.parseEquipment(root);
        assertEquals(1, result.size(), "null slot should be skipped, only chest should remain");
        assertNotNull(result.get("chest"));
    }

    // ── techniques parsing ──────────────────────────────────────────────────

    @Test
    void parsesTechniquesFromCompleteJson() {
        String payload = """
            {"type":"npc_metadata","v":1,"entity_id":20,"archetype":"rogue","realm":"固元",\
            "reputation_to_player":0,"display_name":"测试",\
            "techniques":[{"id":"sword.cleave","display_name":"横斩","proficiency":0.75},\
            {"id":"jiemai.parry","display_name":"截脉弹反","proficiency":0.4}]}
            """.trim();

        assertTrue(NpcMetadataHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length));
        NpcMetadata metadata = NpcMetadataStore.get(20);
        assertNotNull(metadata);
        assertEquals(2, metadata.techniques().size(), "expected 2 techniques");
        assertEquals("sword.cleave", metadata.techniques().get(0).id());
        assertTrue(Math.abs(metadata.techniques().get(0).proficiency() - 0.75) < 0.01,
            "expected proficiency ~0.75 for first technique");
        assertEquals("截脉弹反", metadata.techniques().get(1).displayName());
    }

    @Test
    void parsesTechniquesMissingFieldFallsBackEmpty() {
        String json = """
            {"type":"npc_metadata","v":1,"entity_id":21,"archetype":"beast","realm":"醒灵",\
            "reputation_to_player":0,"display_name":"异兽"}
            """.trim();

        assertTrue(NpcMetadataHandler.handle(json, json.getBytes(StandardCharsets.UTF_8).length));
        NpcMetadata metadata = NpcMetadataStore.get(21);
        assertNotNull(metadata);
        assertTrue(metadata.techniques().isEmpty(),
            "missing techniques field should fallback to empty list");
    }

    @Test
    void parsesTechniquesNullFieldFallsBackEmpty() {
        JsonObject root = JsonParser.parseString("{\"techniques\":null}").getAsJsonObject();
        List<NpcTechniqueData> result = NpcMetadataHandler.parseTechniques(root);
        assertTrue(result.isEmpty(), "null techniques should fallback to empty list");
    }

    @Test
    void parseTechniquesSkipsNullArrayElements() {
        JsonObject root = JsonParser.parseString(
            "{\"techniques\":[null,{\"id\":\"a\",\"display_name\":\"b\",\"proficiency\":0.5}]}"
        ).getAsJsonObject();

        List<NpcTechniqueData> result = NpcMetadataHandler.parseTechniques(root);
        assertEquals(1, result.size(), "null array element should be skipped");
        assertEquals("a", result.get(0).id());
    }

    // ── trade_offers parsing ────────────────────────────────────────────────

    @Test
    void parsesTradeOffersFromCompleteJson() {
        String payload = """
            {"type":"npc_metadata","v":1,"entity_id":30,"archetype":"rogue","realm":"凝脉",\
            "reputation_to_player":10,"display_name":"测试",\
            "trade_offers":[{"template_id":"lingcao","display_name":"灵草","count":3,"price_bone_coins":12}]}
            """.trim();

        assertTrue(NpcMetadataHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length));
        NpcMetadata metadata = NpcMetadataStore.get(30);
        assertNotNull(metadata);
        assertEquals(1, metadata.tradeOffers().size(), "expected 1 trade offer");
        NpcTradeOffer offer = metadata.tradeOffers().get(0);
        assertEquals("lingcao", offer.templateId());
        assertEquals("灵草", offer.displayName());
        assertEquals(3, offer.count());
        assertEquals(12, offer.priceBoneCoins());
    }

    @Test
    void parsesTradeOffersMissingFieldFallsBackEmpty() {
        JsonObject root = JsonParser.parseString("{}").getAsJsonObject();
        List<NpcTradeOffer> result = NpcMetadataHandler.parseTradeOffers(root);
        assertTrue(result.isEmpty(), "missing trade_offers field should fallback to empty list");
    }

    @Test
    void parsesTradeOffersNullFieldFallsBackEmpty() {
        JsonObject root = JsonParser.parseString("{\"trade_offers\":null}").getAsJsonObject();
        List<NpcTradeOffer> result = NpcMetadataHandler.parseTradeOffers(root);
        assertTrue(result.isEmpty(), "null trade_offers should fallback to empty list");
    }

    @Test
    void parseTradeOffersSkipsNullElements() {
        JsonObject root = JsonParser.parseString(
            "{\"trade_offers\":[null,{\"template_id\":\"x\",\"display_name\":\"y\",\"count\":1,\"price_bone_coins\":5}]}"
        ).getAsJsonObject();

        List<NpcTradeOffer> result = NpcMetadataHandler.parseTradeOffers(root);
        assertEquals(1, result.size(), "null array element should be skipped");
        assertEquals("x", result.get(0).templateId());
    }

    // ── tradeCandidate() logic ──────────────────────────────────────────────

    @Test
    void tradeCandidateTrueWhenHasOffersAndNotHostile() {
        NpcMetadata metadata = new NpcMetadata(
            1, "rogue", "凝脉", null, null, 10,
            "散修", "壮年", "你好", null, 1.0, 0.5,
            Map.of(), List.of(),
            List.of(new NpcTradeOffer("lingcao", "灵草", 3, 12))
        );
        assertTrue(metadata.tradeCandidate(),
            "non-hostile NPC with trade offers should be a trade candidate");
    }

    @Test
    void tradeCandidateFalseWhenHostile() {
        NpcMetadata metadata = new NpcMetadata(
            2, "rogue", "凝脉", null, null, -50,
            "散修", "壮年", "你好", null, 1.0, 0.5,
            Map.of(), List.of(),
            List.of(new NpcTradeOffer("lingcao", "灵草", 3, 12))
        );
        assertFalse(metadata.tradeCandidate(),
            "hostile NPC (rep < -30) should not be a trade candidate even with offers");
    }

    @Test
    void tradeCandidateFalseWhenNoOffers() {
        NpcMetadata metadata = new NpcMetadata(
            3, "rogue", "凝脉", null, null, 10,
            "散修", "壮年", "你好", null, 1.0, 0.5,
            Map.of(), List.of(), List.of()
        );
        assertFalse(metadata.tradeCandidate(),
            "NPC with empty trade offers should not be a trade candidate");
    }

    // ── hp/qi ratio clamping ────────────────────────────────────────────────

    @Test
    void hpQiRatioClampedInMetadata() {
        String payload = """
            {"type":"npc_metadata","v":1,"entity_id":40,"archetype":"rogue","realm":"醒灵",\
            "reputation_to_player":0,"display_name":"x","hp_ratio":1.5,"qi_ratio":-0.3}
            """.trim();

        assertTrue(NpcMetadataHandler.handle(payload, payload.getBytes(StandardCharsets.UTF_8).length));
        NpcMetadata metadata = NpcMetadataStore.get(40);
        assertNotNull(metadata);
        assertTrue(metadata.hpRatio() <= 1.0,
            "hp_ratio should be clamped to 1.0 but was " + metadata.hpRatio());
        assertTrue(metadata.qiRatio() >= 0.0,
            "qi_ratio should be clamped to 0.0 but was " + metadata.qiRatio());
    }
}
