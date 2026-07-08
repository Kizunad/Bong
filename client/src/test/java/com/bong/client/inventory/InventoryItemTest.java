package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.component.ItemTooltipPanel;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InventoryItemTest {

    @Test
    void legacyFactoryDefaultsAuthoritativeFields() {
        InventoryItem item = InventoryItem.create(
            "starter_talisman",
            "Starter Talisman",
            1,
            1,
            0.5,
            "common",
            ""
        );

        assertEquals(0L, item.instanceId());
        assertEquals(1, item.stackCount());
        assertEquals(1.0, item.spiritQuality(), 1e-9);
        assertEquals(1.0, item.durability(), 1e-9);
    }

    @Test
    void fullFactoryClampsNewInventoryFields() {
        InventoryItem item = InventoryItem.createFull(
            42L,
            "weathered_pill",
            "Weathered Pill",
            1,
            1,
            0.2,
            "rare",
            "",
            0,
            -0.25,
            1.75
        );

        assertEquals(42L, item.instanceId());
        assertEquals(1, item.stackCount());
        assertEquals(0.0, item.spiritQuality(), 1e-9);
        assertEquals(1.0, item.durability(), 1e-9);
    }

    @Test
    void nanQualityAndDurabilityDefaultToFullValue() {
        InventoryItem item = InventoryItem.createFull(
            43L,
            "unstable_pill",
            "Unstable Pill",
            1,
            1,
            0.2,
            "rare",
            "",
            2,
            Double.NaN,
            Double.NaN
        );

        assertEquals(1.0, item.spiritQuality(), 1e-9);
        assertEquals(1.0, item.durability(), 1e-9);
    }

    @Test
    void boneCoinTooltipUsesSealedQiSemantics() {
        InventoryItem item = InventoryItem.createFull(
            44L,
            "bone_coin_15",
            "封灵骨币",
            1,
            1,
            0.1,
            "common",
            "",
            1,
            0.42,
            1.0
        );

        assertTrue(item.isBoneCoin());
        assertEquals("封灵真元 42%", ItemTooltipPanel.formatStatusLine(item));
    }

    @Test
    void ancientRelicGlowRendersChargesInStatusLine() {
        InventoryItem item = InventoryItem.createFullWithVisualMeta(
            77L,
            "ancient_relic_eye",
            "古眼",
            1,
            1,
            0.8,
            "ancient",
            "",
            1,
            1.0,
            0.9,
            3,
            "",
            "",
            0,
            null,
            "",
            java.util.List.of(),
            null,
            java.util.List.of()
        );

        assertTrue(item.isAncientRelic());
        assertEquals(0xFF4444, item.rarityColor());
        assertEquals("耐久 90%  ⚡ ×3 上古遗物·一次性", ItemTooltipPanel.formatStatusLine(item));
        assertTrue(AncientRelicGlowRenderer.shouldGlow(item));
    }

    @Test
    void ancientRelicChargesClampAndMarkRelic() {
        InventoryItem item = InventoryItem.createFullWithVisualMeta(
            45L,
            "ancient_relic",
            "上古遗物",
            1,
            1,
            0.2,
            "ancient",
            "",
            1,
            0.0,
            1.0,
            8,
            "",
            "",
            0,
            null,
            "",
            java.util.List.of(),
            null,
            java.util.List.of()
        );

        assertTrue(item.isAncientRelic());
        assertEquals(5, item.charges());
        assertEquals(0xFF4444, item.rarityColor());
    }

    @Test
    void rarityMetadataIsNormalizedForVisualSemantics() {
        InventoryItem item = InventoryItem.createFullWithVisualMeta(
            46L,
            "ancient_relic",
            "上古遗物",
            1,
            1,
            0.2,
            " Ancient ",
            "",
            1,
            1.0,
            1.0,
            1,
            "",
            "",
            0,
            null,
            "",
            java.util.List.of(),
            null,
            java.util.List.of()
        );

        assertEquals("ancient", item.rarity());
        assertTrue(item.isAncientRelic());
        assertEquals(0xFF4444, item.rarityColor());
    }

    // ─── plan-rotate-v1 — withRotatedFootprint ──────────────────────────────

    /** 旋转副本：宽高互换，其余字段（含 visualMeta：charges/forge/alchemy）逐字段原样保留。 */
    @Test
    void withRotatedFootprintSwapsDimsAndPreservesEveryOtherField() {
        InventoryItem item = InventoryItem.createFullWithVisualMeta(
            77L,
            "long_rod",
            "长杆",
            2,
            1,
            1.5,
            "rare",
            "旋转测试物",
            1,
            0.8,
            0.9,
            3,
            "skill_scroll",
            "sword.basic",
            120,
            0.75,
            "crimson",
            java.util.List.of("artifact:awakened", "sharp"),
            2,
            java.util.List.of("丹纹一线")
        );

        InventoryItem rotated = item.withRotatedFootprint();

        assertEquals(1, rotated.gridWidth(), "旋转后宽应为原高 1");
        assertEquals(2, rotated.gridHeight(), "旋转后高应为原宽 2");
        assertEquals(item.instanceId(), rotated.instanceId());
        assertEquals(item.itemId(), rotated.itemId());
        assertEquals(item.displayName(), rotated.displayName());
        assertEquals(item.weight(), rotated.weight(), 1e-9);
        assertEquals(item.rarity(), rotated.rarity());
        assertEquals(item.description(), rotated.description());
        assertEquals(item.stackCount(), rotated.stackCount());
        assertEquals(item.spiritQuality(), rotated.spiritQuality(), 1e-9);
        assertEquals(item.durability(), rotated.durability(), 1e-9);
        assertEquals(item.charges(), rotated.charges(), "visualMeta charges 不得丢失");
        assertEquals(item.scrollKind(), rotated.scrollKind());
        assertEquals(item.scrollSkillId(), rotated.scrollSkillId());
        assertEquals(item.scrollXpGrant(), rotated.scrollXpGrant());
        assertEquals(item.forgeQuality(), rotated.forgeQuality(), "forgeQuality 不得丢失");
        assertEquals(item.forgeColor(), rotated.forgeColor());
        assertEquals(item.forgeSideEffects(), rotated.forgeSideEffects(), "forgeSideEffects 不得丢失");
        assertEquals(item.forgeAchievedTier(), rotated.forgeAchievedTier());
        assertEquals(item.alchemyLines(), rotated.alchemyLines(), "alchemyLines 不得丢失");
    }

    // ── plan-forge-session-entry-wiring-v1 修复轮 —— canonical 矿物 id ──────────
    // 起炉曾把 template_id（"mineral_fan_tie"）当材料 key 发给引擎，而 resolve_billet
    // 只认 canonical（"fan_tie"）→ 真人客户端起炉恒 Waste。以下把 key 选择契约锁死。

    @Test
    void mineralIdDefaultsEmptyAndForgeKeyFallsBackToItemId() {
        InventoryItem plain = InventoryItem.simple("iron_sword", "铁剑");
        assertEquals("", plain.mineralId(), "工厂默认 mineralId 应为空串（非矿物）");
        assertEquals(
            "iron_sword",
            plain.forgeMaterialKey(),
            "非矿物件（图谱允许的 item 材料）投料 key 应退回 itemId"
        );
    }

    @Test
    void withMineralIdAttachesCanonicalKeyAndForgeKeyPrefersIt() {
        InventoryItem ore = InventoryItem.simple("mineral_fan_tie", "凡铁")
            .withMineralId("fan_tie");
        assertEquals("fan_tie", ore.mineralId());
        assertEquals(
            "fan_tie",
            ore.forgeMaterialKey(),
            "矿物件投料 key 必须是 canonical mineral_id——发 template_id 会被引擎判 ShortMaterial→Waste"
        );
        assertEquals("mineral_fan_tie", ore.itemId(), "itemId 本身不得被改写");
    }

    @Test
    void withMineralIdNormalizesNullAndBlankToEmpty() {
        assertEquals("", InventoryItem.simple("a", "A").withMineralId(null).mineralId());
        assertEquals("", InventoryItem.simple("a", "A").withMineralId("  ").mineralId());
    }

    @Test
    void withRotatedFootprintPreservesMineralId() {
        InventoryItem ore = InventoryItem.createFull(
            79L, "mineral_fan_tie", "凡铁", 2, 1, 1.0, "common", "", 3, 1.0, 1.0)
            .withMineralId("fan_tie");
        assertEquals(
            "fan_tie",
            ore.withRotatedFootprint().mineralId(),
            "旋转副本不得丢 mineralId——丢了会让已选投料退回 template_id key"
        );
    }

    @Test
    void equalsDistinguishesMineralId() {
        InventoryItem bare = InventoryItem.simple("mineral_fan_tie", "凡铁");
        InventoryItem tagged = bare.withMineralId("fan_tie");
        assertTrue(!bare.equals(tagged), "mineralId 不同的两件不得判等（diff 刷新依赖 equals）");
        assertEquals(tagged, bare.withMineralId("fan_tie"));
    }

    /** 连转两次 = 恢复原朝向（equals 校验全字段一致）。 */
    @Test
    void withRotatedFootprintTwiceRestoresOriginal() {
        InventoryItem item = InventoryItem.createFull(
            78L, "long_rod", "长杆", 2, 1, 1.0, "common", "", 1, 1.0, 1.0);
        assertEquals(item, item.withRotatedFootprint().withRotatedFootprint(),
            "旋转两次应与原件完全相等（宽高恢复且其余字段未漂移）");
    }

    /** 正方形（含 1x1）旋转是恒等操作：直接返回自身实例。 */
    @Test
    void withRotatedFootprintIsIdentityForSquareItems() {
        InventoryItem oneByOne = InventoryItem.createFull(
            79L, "pebble", "石子", 1, 1, 0.1, "common", "", 1, 1.0, 1.0);
        assertTrue(oneByOne == oneByOne.withRotatedFootprint(),
            "1x1 旋转应返回自身（no-op）");

        InventoryItem twoByTwo = InventoryItem.createFull(
            80L, "crate", "木箱", 2, 2, 2.0, "common", "", 1, 1.0, 1.0);
        assertTrue(twoByTwo == twoByTwo.withRotatedFootprint(),
            "2x2 正方形旋转应返回自身（no-op）");
    }
}
