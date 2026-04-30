package com.bong.client.insight;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class InsightOfferViewModelTest {
    @Test
    void mockOfferHasThreeChoicesCoveringDistinctCategories() {
        InsightOfferViewModel offer = MockInsightOfferData.firstInduceBreakthrough();

        assertEquals(3, offer.choices().size());
        assertEquals(InsightCategory.BREAKTHROUGH, offer.choices().get(0).category());
        assertEquals(InsightCategory.COMPOSURE, offer.choices().get(1).category());
        assertEquals(InsightCategory.PERCEPTION, offer.choices().get(2).category());
    }

    @Test
    void remainingMillisClampsToZeroAfterExpiry() {
        InsightOfferViewModel offer = MockInsightOfferData.firstInduceBreakthrough(1_000L);

        assertEquals(500L, offer.remainingMillis(500L));
        assertEquals(0L, offer.remainingMillis(1_000L));
        assertEquals(0L, offer.remainingMillis(2_000L));
        assertTrue(offer.isExpired(1_000L));
        assertFalse(offer.isExpired(999L));
    }

    @Test
    void rejectsEmptyChoiceList() {
        assertThrows(IllegalArgumentException.class, () -> new InsightOfferViewModel(
            "trig", "trig label", "realm", 0.5, 1, 1,
            System.currentTimeMillis() + 1000L,
            List.of()
        ));
    }

    @Test
    void describeRendersTriggerHeaderAndAllChoices() {
        InsightOfferScreen.RenderContent content = InsightOfferScreen.describe(
            MockInsightOfferData.firstInduceBreakthrough());

        assertEquals("◇ 心 有 所 感 ◇", content.lines().get(0));
        assertEquals("【触发】首次突破到引气境", content.lines().get(1));
        assertEquals("境界: 引气境 (3 正经)", content.lines().get(2));
        assertEquals("剩余顿悟额度: 2/2", content.lines().get(4));
        assertTrue(content.lines().get(5).startsWith("⏳ "));
        assertTrue(content.lines().contains("[E] 下次冲关稳"));
        assertTrue(content.lines().contains("[C] 闭关心如止"));
        assertTrue(content.lines().contains("[G] 灵气浓淡可见"));
        assertTrue(content.lines().stream().anyMatch(l -> l.contains("✦ 你已知冲关时神识凝聚的诀窍")));
        assertEquals("[ 心未契机 ]", content.lines().get(content.lines().size() - 1));
    }

    @Test
    void describeRendersHeartDemonSpecificCopy() {
        InsightOfferScreen.RenderContent content = InsightOfferScreen.describe(
            MockInsightOfferData.heartDemonOffer());

        assertEquals("◇ 心 魔 劫 ◇", content.lines().get(0));
        assertEquals("【触发】心魔劫临身", content.lines().get(1));
        assertEquals("境界: 渡虚劫 · 心魔", content.lines().get(2));
        assertEquals("心魔抉择: 3 项", content.lines().get(4));
        assertTrue(content.lines().get(5).startsWith("心魔倒计时: "));
        assertTrue(content.lines().get(5).contains("超时默认执念"));
        assertTrue(content.lines().contains("[C] 守本心"));
        assertTrue(content.lines().contains("[E] 斩执念"));
        assertTrue(content.lines().contains("[G] 无解"));
        assertEquals("[ 不作答 ]", content.lines().get(content.lines().size() - 1));
        assertFalse(content.lines().stream().anyMatch(l -> l.contains("顿悟额度")));
        assertFalse(content.lines().stream().anyMatch(l -> l.contains("心未契机")));
    }

    @Test
    void categoryAccentColorsAreDistinct() {
        InsightCategory[] all = InsightCategory.values();
        for (int i = 0; i < all.length; i++) {
            for (int j = i + 1; j < all.length; j++) {
                assertFalse(all[i].accentArgb() == all[j].accentArgb(),
                    "重色: " + all[i] + " vs " + all[j]);
            }
        }
    }
}
