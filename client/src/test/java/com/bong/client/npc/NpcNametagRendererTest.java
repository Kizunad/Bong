package com.bong.client.npc;

import com.bong.client.spider.SpiderDisguiseHandler;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NpcNametagRendererTest {
    @BeforeEach
    void clearSpiderDisguiseState() {
        SpiderDisguiseHandler.clearOnDisconnect();
    }

    @Test
    void nametagColorTracksReputation() {
        assertEquals(0xE05A47, NpcNametagRenderer.colorByReputation(-31));
        assertEquals(0xC8C8C8, NpcNametagRenderer.colorByReputation(-30));
        assertEquals(0xC8C8C8, NpcNametagRenderer.colorByReputation(0));
        assertEquals(0xC8C8C8, NpcNametagRenderer.colorByReputation(50));
        assertEquals(0x5DD17A, NpcNametagRenderer.colorByReputation(51));
    }

    @Test
    void distanceLabelFallsBackToIconThenHides() {
        NpcMetadata metadata = new NpcMetadata(
            42,
            "rogue",
            "凝脉",
            null,
            null,
            0,
            "散修·凝脉",
            "正值壮年",
            "道友，可有灵草出让？",
            "真元流转平稳"
        );

        assertEquals("[散修·凝脉]", NpcNametagRenderer.labelForDistance(metadata, 19.0, "凝脉"));
        assertEquals("散", NpcNametagRenderer.labelForDistance(metadata, 25.0, "凝脉"));
        assertEquals("", NpcNametagRenderer.labelForDistance(metadata, 40.0, "凝脉"));
        assertEquals("[散修·凝脉]", NpcNametagRenderer.labelForDistance(metadata, 19.0, "引气"));
        assertTrue(NpcNametagRenderer.labelForDistance(metadata, 19.0, "Awaken").startsWith("⚠ "));
        assertEquals("[散修·凝脉]", NpcNametagRenderer.labelForDistance(metadata, 19.0, "Induce"));
    }

    @Test
    void discipleDistanceIconUsesRuinedRemnantGlyph() {
        NpcMetadata metadata = new NpcMetadata(
            43,
            "disciple",
            "凝脉",
            null,
            null,
            0,
            "残宗余孽·凝脉",
            "正值壮年",
            "道友，可有灵草出让？",
            null
        );

        assertEquals("[残宗余孽·凝脉]", NpcNametagRenderer.labelForDistance(metadata, 19.0, "凝脉"));
        assertEquals("余", NpcNametagRenderer.labelForDistance(metadata, 25.0, "凝脉"));
    }

    @Test
    void nameVisibleFalseHidesBeastMetadataLabels() {
        NpcMetadata metadata = metadata(42, "beast", "妖兽·醒灵", false);

        assertEquals(
            "",
            NpcNametagRenderer.labelForDistance(metadata, 19.0, "醒灵"),
            "NameVisible(false) 下发为 nametag_visible=false 时，近距离完整名牌也必须隐藏"
        );
        assertEquals(
            "",
            NpcNametagRenderer.labelForDistance(metadata, 25.0, "醒灵"),
            "NameVisible(false) 不能被 Beast 的远距离“兽”图标覆盖"
        );
    }

    @Test
    void disguisedSpiderSuppressesMetadataLabelUntilAmbush() {
        NpcMetadata metadata = metadata(42, "beast", "妖兽·醒灵", true);
        String enter = """
            {"v":1,"type":"spider_disguise_enter","entity_ids":[42]}
            """;
        SpiderDisguiseHandler.handleEnter(enter, enter.getBytes().length);

        assertEquals(
            "",
            NpcNametagRenderer.labelForEntity(
                metadata,
                19.0,
                "醒灵",
                SpiderDisguiseHandler.isDisguised(42)
            ),
            "Disguised 蛛即使已有 NpcMetadata，也不能显示 [妖兽·醒灵]"
        );

        String ambush = """
            {"v":1,"type":"spider_ambush_trigger","entity_ids":[42]}
            """;
        SpiderDisguiseHandler.handleAmbush(ambush, ambush.getBytes().length);

        assertEquals(
            "[妖兽·醒灵]",
            NpcNametagRenderer.labelForEntity(
                metadata,
                19.0,
                "醒灵",
                SpiderDisguiseHandler.isDisguised(42)
            ),
            "Ambush 后伪装解除，metadata 名牌应恢复"
        );
    }

    @Test
    void ambushedSpiderShowsLabelEvenWhenMetadataStillHiddenThenDeltaEnterHidesAgain() {
        NpcMetadata staleDisguisedMetadata = metadata(42, "beast", "妖兽·醒灵", false);
        String enter = """
            {"v":1,"type":"spider_disguise_enter","entity_ids":[42]}
            """;
        SpiderDisguiseHandler.handleEnter(enter, enter.getBytes().length);

        assertEquals(
            "",
            NpcNametagRenderer.labelForEntity(staleDisguisedMetadata, 19.0, "醒灵", 42),
            "Disguised 期 metadata hidden 时应隐藏名牌"
        );

        String ambush = """
            {"v":1,"type":"spider_ambush_trigger","entity_ids":[42]}
            """;
        SpiderDisguiseHandler.handleAmbush(ambush, ambush.getBytes().length);

        assertEquals(
            "[妖兽·醒灵]",
            NpcNametagRenderer.labelForEntity(staleDisguisedMetadata, 19.0, "醒灵", 42),
            "Ambush 后即使 metadata 尚未刷新，也应通过 revealed 状态恢复名牌"
        );

        String deltaEnter = """
            {"v":1,"type":"spider_disguise_enter","entity_ids":[42],"full_sync":false}
            """;
        SpiderDisguiseHandler.handleEnter(deltaEnter, deltaEnter.getBytes().length);

        assertEquals(
            "",
            NpcNametagRenderer.labelForEntity(staleDisguisedMetadata, 19.0, "醒灵", 42),
            "重新 Disguised 的增量 enter 必须立即隐藏名牌，不能等周期 full sync"
        );
    }

    private static NpcMetadata metadata(
        int entityId,
        String archetype,
        String displayName,
        boolean nametagVisible
    ) {
        return new NpcMetadata(
            entityId,
            archetype,
            "醒灵",
            null,
            null,
            0,
            displayName,
            nametagVisible,
            "正值壮年",
            "它盯着你，喉间低鸣。",
            null,
            1.0,
            0.0,
            Map.of(),
            List.of(),
            List.of()
        );
    }
}
