package com.bong.client.npc;

import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;
import java.util.regex.Pattern;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Unit tests for NpcDialogueScreen / NpcInspectScreen / NpcTradeScreen
 * using their pure-function {@code describe()} methods.
 * (No real MC client needed -- pure data → layout description.)
 */
public class NpcScreenDescribeTest {
    private static final Pattern SECT_WORDS = Pattern.compile(".*(魔修|正道|宗门|掌门|真传弟子|内门|外门).*", Pattern.DOTALL);

    // ── helpers ─────────────────────────────────────────────────────────────

    private static NpcMetadata friendly() {
        return new NpcMetadata(
            1, "rogue", "凝脉", "中立盟", "客卿", 55,
            "张三丰", "壮年", "道友远来，不知有何贵干？", "真元流转平稳",
            0.95, 0.8,
            Map.of(
                "main_hand", new NpcEquipSlotData("iron_sword", "铁剑", 0, 0.85),
                "chest", new NpcEquipSlotData("bone_chestplate", "骨甲", 1, 0.7)
            ),
            List.of(
                new NpcTechniqueData("sword.cleave", "基础横斩", 0.75),
                new NpcTechniqueData("jiemai.parry", "截脉弹反", 0.4)
            ),
            List.of(
                new NpcTradeOffer("lingcao", "灵草", 3, 12),
                new NpcTradeOffer("fragment_scroll", "残卷", 1, 45)
            )
        );
    }

    private static NpcMetadata hostile() {
        return new NpcMetadata(
            2, "daoxiang", "引气", null, null, -50,
            "道伥·引气", "风烛残年", "此人气息浑浊。", null,
            0.4, 0.1,
            Map.of("main_hand", new NpcEquipSlotData("rusted_sword", "锈剑", 0, 0.3)),
            List.of(new NpcTechniqueData("wild.claw", "乱爪", 0.2)),
            List.of(new NpcTradeOffer("bone_shard", "碎骨", 5, 2))
        );
    }

    private static NpcMetadata noEquipNoTech() {
        return new NpcMetadata(
            3, "beast", "醒灵", null, null, 0,
            "妖兽·醒灵", "初入尘世", "它盯着你。", null,
            1.0, 0.0,
            Map.of(), List.of(), List.of()
        );
    }

    // ── NpcDialogueScreen.describe ──────────────────────────────────────────

    @Test
    void dialogueNullMetadataReturnsPlaceholder() {
        NpcDialogueScreen.RenderContent content = NpcDialogueScreen.describe(null);
        assertTrue(content.placeholder(), "null metadata should produce placeholder");
        assertTrue(content.lines().contains("null metadata"));
    }

    @Test
    void dialogueFriendlyShowsTradeOption() {
        NpcDialogueScreen.RenderContent content = NpcDialogueScreen.describe(friendly());
        assertFalse(content.placeholder());
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("看看你有什么好东西")),
            "friendly NPC with trade offers should show trade option");
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("让我打量一下你")),
            "inspect option should always be visible");
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("告辞")),
            "leave option should always be visible");
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("信任")),
            "rep > 50 should show '信任' label");
    }

    @Test
    void dialogueHostileHidesTradeShowsWarning() {
        NpcDialogueScreen.RenderContent content = NpcDialogueScreen.describe(hostile());
        assertFalse(content.placeholder());
        // hostile should show hostile greeting, not trade option
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("此人对你充满敌意")),
            "hostile NPC should show hostile greeting text");
        assertFalse(content.lines().stream().anyMatch(l -> l.contains("看看你有什么好东西")),
            "hostile NPC should NOT show trade option even if tradeOffers exist");
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("让我打量一下你")),
            "inspect option should still be visible for hostile NPC");
    }

    @Test
    void dialogueNoTradeOffersHidesTradeOption() {
        NpcDialogueScreen.RenderContent content = NpcDialogueScreen.describe(noEquipNoTech());
        assertFalse(content.lines().stream().anyMatch(l -> l.contains("看看你有什么好东西")),
            "NPC with no trade offers should not show trade option");
    }

    @Test
    void dialogueArchetypeLabelMapping() {
        assertEquals("散修", NpcDialogueScreen.archetypeLabel("rogue"));
        assertEquals("凡人", NpcDialogueScreen.archetypeLabel("commoner"));
        assertEquals("残宗余孽", NpcDialogueScreen.archetypeLabel("disciple"));
        assertEquals("异兽", NpcDialogueScreen.archetypeLabel("beast"));
        assertEquals("游尸", NpcDialogueScreen.archetypeLabel("zombie"));
        assertEquals("遗迹守卫", NpcDialogueScreen.archetypeLabel("guardian_relic"));
        assertEquals("道伥", NpcDialogueScreen.archetypeLabel("daoxiang"));
        assertEquals("执念", NpcDialogueScreen.archetypeLabel("zhinian"));
        assertEquals("负压畸变体", NpcDialogueScreen.archetypeLabel("fuya"));
        assertEquals("骨煞", NpcDialogueScreen.archetypeLabel("skull_fiend"));
        assertEquals("未知", NpcDialogueScreen.archetypeLabel("nonexistent"));
        assertEquals("未知", NpcDialogueScreen.archetypeLabel(null));
    }

    @Test
    void dialogueRealmBadge() {
        assertEquals("· 凝脉", NpcDialogueScreen.realmBadge("凝脉"));
        assertEquals("· 未知", NpcDialogueScreen.realmBadge(null));
        assertEquals("· 未知", NpcDialogueScreen.realmBadge(""));
    }

    // ── NpcInspectScreen.describe ───────────────────────────────────────────

    @Test
    void inspectNullMetadataReturnsPlaceholder() {
        NpcInspectScreen.RenderContent content = NpcInspectScreen.describe(null, 3);
        assertTrue(content.placeholder(), "null metadata should produce placeholder");
    }

    @Test
    void inspectShowsEquipmentDetails() {
        NpcInspectScreen.RenderContent content = NpcInspectScreen.describe(friendly(), NpcInspectScreen.REALM_RANK_FULL);
        assertFalse(content.placeholder());
        assertTrue(content.lines().stream().anyMatch(l -> l.startsWith("equip:") && l.contains("铁剑")),
            "inspect should show main_hand equipment with display name");
        assertTrue(content.lines().stream().anyMatch(l -> l.startsWith("equip:") && l.contains("骨甲")),
            "inspect should show chest equipment with display name");
    }

    @Test
    void inspectEmptyEquipmentShowsNone() {
        NpcInspectScreen.RenderContent content = NpcInspectScreen.describe(noEquipNoTech(), NpcInspectScreen.REALM_RANK_FULL);
        assertTrue(content.lines().stream().anyMatch(l -> l.equals("equip: 无")),
            "empty equipment should show '无'");
    }

    @Test
    void inspectTechniquesUnknownForLowRealm() {
        // 醒灵/引气 (rank <= 1) should see "未知"
        NpcInspectScreen.RenderContent content = NpcInspectScreen.describe(friendly(), NpcInspectScreen.REALM_RANK_UNKNOWN);
        assertTrue(content.lines().stream().anyMatch(l -> l.equals("tech: 未知")),
            "player at 醒灵/引气 rank should see '未知' for techniques");
        assertFalse(content.lines().stream().anyMatch(l -> l.contains("基础横斩")),
            "technique names should be hidden at low realm");
    }

    @Test
    void inspectTechniquesNameOnlyForNingmai() {
        // 凝脉 (rank = 2) should see name but NOT proficiency
        NpcInspectScreen.RenderContent content = NpcInspectScreen.describe(friendly(), NpcInspectScreen.REALM_RANK_NAME_ONLY);
        assertTrue(content.lines().stream().anyMatch(l -> l.equals("tech: 基础横斩")),
            "player at 凝脉 should see technique name only");
        assertTrue(content.lines().stream().anyMatch(l -> l.equals("tech: 截脉弹反")),
            "player at 凝脉 should see second technique name only");
        assertFalse(content.lines().stream().anyMatch(l -> l.contains("0.75")),
            "proficiency should NOT be visible at 凝脉 rank");
    }

    @Test
    void inspectTechniquesFullForGuyuan() {
        // 固元+ (rank >= 3) should see name + proficiency
        NpcInspectScreen.RenderContent content = NpcInspectScreen.describe(friendly(), NpcInspectScreen.REALM_RANK_FULL);
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("基础横斩") && l.contains("0.75")),
            "player at 固元+ should see technique name with proficiency");
    }

    @Test
    void inspectEmptyTechniquesShowsNone() {
        NpcInspectScreen.RenderContent content = NpcInspectScreen.describe(noEquipNoTech(), NpcInspectScreen.REALM_RANK_FULL);
        assertTrue(content.lines().stream().anyMatch(l -> l.equals("tech: 无")),
            "NPC with no techniques should show '无' regardless of player realm");
    }

    @Test
    void inspectShowsAttitudeInsteadOfFactionWhenFactionFieldsArePresent() {
        NpcInspectScreen.RenderContent content = NpcInspectScreen.describe(friendly(), NpcInspectScreen.REALM_RANK_FULL);

        assertTrue(content.lines().stream().anyMatch(l -> l.equals("attitude: 亲善")),
            "inspect should show attitude from reputation_to_player instead of faction fields");
        assertFalse(content.lines().stream().anyMatch(l -> l.startsWith("faction:")),
            "inspect should not render faction lines after NPC faction anonymity reframe");
        assertInspectHasNoSectWords(content);
    }

    @Test
    void inspectNoFactionStillShowsNeutralAttitude() {
        NpcInspectScreen.RenderContent content = NpcInspectScreen.describe(noEquipNoTech(), NpcInspectScreen.REALM_RANK_FULL);

        assertTrue(content.lines().stream().anyMatch(l -> l.equals("attitude: 中立")),
            "NPC without faction should still show neutral attitude");
        assertFalse(content.lines().stream().anyMatch(l -> l.startsWith("faction:")),
            "inspect should not show fallback faction lines after anonymity reframe");
    }

    @Test
    void inspectAttitudeLabels() {
        assertEquals("亲善", NpcInspectScreen.attitudeLabel(34));
        assertEquals("亲善", NpcInspectScreen.attitudeLabel(100));
        assertEquals("敌意", NpcInspectScreen.attitudeLabel(-34));
        assertEquals("敌意", NpcInspectScreen.attitudeLabel(-100));
        assertEquals("中立", NpcInspectScreen.attitudeLabel(-33));
        assertEquals("中立", NpcInspectScreen.attitudeLabel(33));
        assertEquals("中立", NpcInspectScreen.attitudeLabel(0));
    }

    @Test
    void npcIdentitySurfacesDoNotContainSectWords() {
        NpcMetadata metadata = new NpcMetadata(
            4, "disciple", "凝脉", "魔修派", "掌门", 40,
            "残宗余孽·凝脉", "正值壮年", "道友，可有灵草出让？", null,
            1.0, 0.5,
            Map.of(), List.of(), List.of()
        );

        NpcDialogueScreen.RenderContent dialogue = NpcDialogueScreen.describe(metadata);
        NpcInspectScreen.RenderContent inspect = NpcInspectScreen.describe(metadata, NpcInspectScreen.REALM_RANK_FULL);

        assertNoSectWords(String.join("\n", dialogue.lines()), "dialogue");
        assertInspectHasNoSectWords(inspect);
    }

    @Test
    void inspectRealmToRankMapping() {
        assertEquals(0, NpcInspectScreen.realmToRank("醒灵"));
        assertEquals(1, NpcInspectScreen.realmToRank("引气"));
        assertEquals(2, NpcInspectScreen.realmToRank("凝脉"));
        assertEquals(3, NpcInspectScreen.realmToRank("固元"));
        assertEquals(4, NpcInspectScreen.realmToRank("通灵"));
        assertEquals(5, NpcInspectScreen.realmToRank("化虚"));
        assertEquals(0, NpcInspectScreen.realmToRank(null));
        assertEquals(0, NpcInspectScreen.realmToRank(""));
        assertEquals(0, NpcInspectScreen.realmToRank("unknown_realm"));
    }

    // ── NpcTradeScreen.describe ─────────────────────────────────────────────

    @Test
    void tradeNullMetadataReturnsPlaceholder() {
        NpcTradeScreen.RenderContent content = NpcTradeScreen.describe(null);
        assertTrue(content.placeholder(), "null metadata should produce placeholder");
    }

    @Test
    void tradeShowsAllOffers() {
        NpcTradeScreen.RenderContent content = NpcTradeScreen.describe(friendly());
        assertFalse(content.placeholder());
        long wareCount = content.lines().stream().filter(l -> l.startsWith("ware:")).count();
        assertEquals(2, wareCount, "expected 2 ware lines for 2 trade offers");
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("灵草") && l.contains("12")),
            "first offer should show name and price");
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("残卷") && l.contains("45")),
            "second offer should show name and price");
    }

    @Test
    void tradeEmptyOffersShowsNoWares() {
        NpcTradeScreen.RenderContent content = NpcTradeScreen.describe(noEquipNoTech());
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("无可售物品")),
            "empty trade offers should show '无可售物品'");
    }

    @Test
    void tradeDiscountShownForHighReputation() {
        // rep > 50 should show discount
        NpcTradeScreen.RenderContent content = NpcTradeScreen.describe(friendly());
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("discount") || l.contains("8折")),
            "rep > 50 should show discount line");
    }

    @Test
    void tradeNoDiscountForLowReputation() {
        NpcTradeScreen.RenderContent content = NpcTradeScreen.describe(noEquipNoTech());
        assertFalse(content.lines().stream().anyMatch(l -> l.contains("discount")),
            "rep <= 50 should NOT show discount line");
    }

    // ── NpcEquipSlotData record ─────────────────────────────────────────────

    @Test
    void equipSlotQualityColorAndLabel() {
        NpcEquipSlotData common = new NpcEquipSlotData("x", "y", 0, 1.0);
        assertEquals(0xC8C8C8, common.qualityColor(), "tier 0 should be grey (凡铁)");
        assertEquals("凡铁", common.qualityLabel());

        NpcEquipSlotData spirit = new NpcEquipSlotData("x", "y", 1, 1.0);
        assertEquals(0x5DD17A, spirit.qualityColor(), "tier 1 should be green (灵器)");
        assertEquals("灵器", spirit.qualityLabel());

        NpcEquipSlotData treasure = new NpcEquipSlotData("x", "y", 2, 1.0);
        assertEquals(0x4A9EE0, treasure.qualityColor(), "tier 2 should be blue (法宝)");
        assertEquals("法宝", treasure.qualityLabel());
    }

    @Test
    void equipSlotDurabilityColorThresholds() {
        NpcEquipSlotData high = new NpcEquipSlotData("x", "y", 0, 0.8);
        assertEquals(0x5DD17A, high.durabilityColor(), "> 0.5 should be green");

        NpcEquipSlotData mid = new NpcEquipSlotData("x", "y", 0, 0.35);
        assertEquals(0xE2C84A, mid.durabilityColor(), "0.2-0.5 should be yellow");

        NpcEquipSlotData low = new NpcEquipSlotData("x", "y", 0, 0.1);
        assertEquals(0xCC4444, low.durabilityColor(), "< 0.2 should be red");
    }

    @Test
    void equipSlotClampsValues() {
        NpcEquipSlotData over = new NpcEquipSlotData("x", "y", 5, 1.5);
        assertEquals(3, over.qualityTier(), "quality_tier should be clamped to max 3");
        assertTrue(over.durabilityRatio() <= 1.0, "durability should be clamped to 1.0");

        NpcEquipSlotData under = new NpcEquipSlotData("x", "y", -1, -0.5);
        assertEquals(0, under.qualityTier(), "quality_tier should be clamped to min 0");
        assertTrue(under.durabilityRatio() >= 0.0, "durability should be clamped to 0.0");
    }

    @Test
    void equipSlotNullDisplayNameFallsBackToTemplateId() {
        NpcEquipSlotData slot = new NpcEquipSlotData("iron_sword", null, 0, 1.0);
        assertEquals("iron_sword", slot.displayName(),
            "null display_name should fallback to template_id");
    }

    // ── NpcTechniqueData record ─────────────────────────────────────────────

    @Test
    void techniqueDataProficiencyColorThresholds() {
        NpcTechniqueData high = new NpcTechniqueData("a", "b", 0.8);
        assertEquals(0x5DD17A, high.proficiencyColor(), "> 0.6 should be green");

        NpcTechniqueData mid = new NpcTechniqueData("a", "b", 0.45);
        assertEquals(0xE2C84A, mid.proficiencyColor(), "0.3-0.6 should be yellow");

        NpcTechniqueData low = new NpcTechniqueData("a", "b", 0.1);
        assertEquals(0xCC4444, low.proficiencyColor(), "< 0.3 should be red");
    }

    @Test
    void techniqueDataClampsProficiency() {
        NpcTechniqueData over = new NpcTechniqueData("a", "b", 1.5);
        assertTrue(over.proficiency() <= 1.0, "proficiency should be clamped to 1.0");

        NpcTechniqueData under = new NpcTechniqueData("a", "b", -0.5);
        assertTrue(under.proficiency() >= 0.0, "proficiency should be clamped to 0.0");
    }

    @Test
    void techniqueDataNullDisplayNameFallsBackToId() {
        NpcTechniqueData tech = new NpcTechniqueData("sword.cleave", null, 0.5);
        assertEquals("sword.cleave", tech.displayName(),
            "null display_name should fallback to id");
    }

    // ── NpcTradeOffer record ────────────────────────────────────────────────

    @Test
    void tradeOfferClampsNegativeValues() {
        NpcTradeOffer offer = new NpcTradeOffer("x", "y", -5, -10);
        assertEquals(0, offer.count(), "negative count should be clamped to 0");
        assertEquals(0, offer.priceBoneCoins(), "negative price should be clamped to 0");
    }

    @Test
    void tradeOfferNullDisplayNameFallsBackToTemplateId() {
        NpcTradeOffer offer = new NpcTradeOffer("lingcao", null, 1, 5);
        assertEquals("lingcao", offer.displayName(),
            "null display_name should fallback to template_id");
    }

    // ── inspect bar/format helpers ──────────────────────────────────────────

    @Test
    void buildBarLength() {
        String bar = NpcInspectScreen.buildBar(0.5);
        assertEquals(NpcInspectScreen.BAR_SEGMENTS, bar.length(),
            "bar should always have BAR_SEGMENTS characters");
        long filled = bar.chars().filter(c -> c == '█').count();
        assertEquals(5, filled, "50% should fill 5 of 10 segments");
    }

    @Test
    void buildBarEdgeCases() {
        String full = NpcInspectScreen.buildBar(1.0);
        assertEquals(NpcInspectScreen.BAR_SEGMENTS, full.chars().filter(c -> c == '█').count(),
            "100% should fill all segments");

        String empty = NpcInspectScreen.buildBar(0.0);
        assertEquals(0, empty.chars().filter(c -> c == '█').count(),
            "0% should fill no segments");

        String clamped = NpcInspectScreen.buildBar(2.0);
        assertEquals(NpcInspectScreen.BAR_SEGMENTS, clamped.chars().filter(c -> c == '█').count(),
            "> 1.0 should be clamped to 100%");
    }

    @Test
    void formatPercent() {
        assertEquals("50%", NpcInspectScreen.formatPercent(0.5));
        assertEquals("0%", NpcInspectScreen.formatPercent(0.0));
        assertEquals("100%", NpcInspectScreen.formatPercent(1.0));
        assertEquals("100%", NpcInspectScreen.formatPercent(1.5));
    }

    @Test
    void slotLabelMapping() {
        assertEquals("主手", NpcInspectScreen.slotLabel("main_hand"));
        assertEquals("副手", NpcInspectScreen.slotLabel("off_hand"));
        assertEquals("头盔", NpcInspectScreen.slotLabel("head"));
        assertEquals("胸甲", NpcInspectScreen.slotLabel("chest"));
        assertEquals("腿甲", NpcInspectScreen.slotLabel("legs"));
        assertEquals("靴子", NpcInspectScreen.slotLabel("feet"));
        assertEquals("unknown_slot", NpcInspectScreen.slotLabel("unknown_slot"));
    }

    private static void assertInspectHasNoSectWords(NpcInspectScreen.RenderContent content) {
        assertNoSectWords(String.join("\n", content.lines()), "inspect");
    }

    private static void assertNoSectWords(String actual, String surface) {
        assertFalse(SECT_WORDS.matcher(actual).matches(),
            "期望 NPC " + surface + " 不含具名宗门字样（reframe b），实际: " + actual);
    }
}
