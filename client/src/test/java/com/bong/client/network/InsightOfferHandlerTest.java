package com.bong.client.network;

import com.bong.client.insight.InsightCategory;
import com.bong.client.insight.InsightOfferStore;
import com.bong.client.insight.InsightOfferViewModel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * InsightOfferHandler 单测（plan-exploration-probe-return-v1 P2）。
 *
 * <p>测试策略：
 * <ul>
 *   <li>正常 offer → InsightOfferStore 快照字段断言（triggerId/triggerLabel/realmLabel/composure/choices）。</li>
 *   <li>每个代表性 InsightChoiceV1 的合成结果：deriveTitle / deriveEffectSummary / categoryZh / effectKindTail。</li>
 *   <li>含 cost_* 与不含 cost_* 两态。</li>
 *   <li>畸形 payload 守卫（缺 trigger_id → noOp；choices 空 → noOp）。</li>
 * </ul>
 */
public class InsightOfferHandlerTest {

    @AfterEach
    void resetStore() {
        InsightOfferStore.resetForTests();
    }

    // ─── 正常 offer：store 被填 ───────────────────────────────────────────────

    @Test
    void normalOffer_populatesInsightOfferStore() {
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "trigger_id": "awaken_first",
              "choices": [
                {
                  "category": "Qi",
                  "effect_kind": "qi_max",
                  "magnitude": 0.05,
                  "flavor_text": "气海微微扩张。",
                  "alignment": "converge"
                }
              ]
            }
            """;
        ServerDataEnvelope envelope = parse(json);

        ServerDataDispatch dispatch = new InsightOfferHandler().handle(envelope);

        assertTrue(dispatch.handled(),
            "正常 offer 应返回 dispatch.handled()==true，实际=" + dispatch.handled());

        InsightOfferViewModel offer = InsightOfferStore.snapshot();
        assertNotNull(offer, "handler 应将 offer 写入 InsightOfferStore，snapshot 不应为 null");
        assertEquals("awaken_first", offer.triggerId(),
            "triggerId 应 round-trip 为 'awaken_first'，实际=" + offer.triggerId());
        assertEquals("顿悟临身", offer.triggerLabel(),
            "triggerLabel 应为 InsightOfferHandler.DEFAULT_TRIGGER_LABEL='顿悟临身'，实际=" + offer.triggerLabel());
        assertEquals("道机一现", offer.realmLabel(),
            "realmLabel 应为 DEFAULT_REALM_LABEL='道机一现'，实际=" + offer.realmLabel());
        assertEquals(0.75d, offer.composure(), 0.001,
            "composure 应为 DEFAULT_COMPOSURE=0.75，实际=" + offer.composure());
        assertEquals(1, offer.quotaRemaining(),
            "quotaRemaining 应为 DEFAULT_QUOTA_REMAINING=1，实际=" + offer.quotaRemaining());
        assertEquals(3, offer.quotaTotal(),
            "quotaTotal 应为 DEFAULT_QUOTA_TOTAL=3，实际=" + offer.quotaTotal());
        assertFalse(offer.choices().isEmpty(),
            "choices 不应为空");
    }

    // ─── choice 合成：deriveTitle / deriveEffectSummary ────────────────────────

    @Test
    void deriveTitle_qiAndQiMax_returnsQiAndUnlock() {
        // category="Qi" → "真元"，effect_kind="qi_max" → tail="解封" → "真元 · 解封"
        String title = InsightOfferHandler.deriveTitle("Qi", "qi_max");
        assertEquals("真元 · 解封", title,
            "category=Qi + effect_kind=qi_max 应合成 '真元 · 解封'，实际=" + title);
    }

    @Test
    void deriveTitle_meridianAndRate_returnsMeridianAndSpeed() {
        // category="Meridian" → "经脉"，effect_kind="meridian_rate" → tail="通速"
        String title = InsightOfferHandler.deriveTitle("Meridian", "meridian_rate");
        assertEquals("经脉 · 通速", title,
            "category=Meridian + effect_kind=meridian_rate 应合成 '经脉 · 通速'，实际=" + title);
    }

    @Test
    void deriveTitle_composureAndRecover_returnsComposureAndRecover() {
        String title = InsightOfferHandler.deriveTitle("Composure", "composure_recover");
        assertEquals("心境 · 回复", title,
            "category=Composure + effect_kind=composure_recover 应合成 '心境 · 回复'，实际=" + title);
    }

    @Test
    void deriveTitle_perceptionCategory_returnsPerceptionLabel() {
        String title = InsightOfferHandler.deriveTitle("Perception", "perception_bonus");
        assertEquals("感知 · 增幅", title,
            "category=Perception + effect_kind=perception_bonus 应合成 '感知 · 增幅'，实际=" + title);
    }

    @Test
    void deriveTitle_breakthroughCategory_returnsBreakthroughLabel() {
        String title = InsightOfferHandler.deriveTitle("Breakthrough", "breakthrough_drop");
        assertEquals("突破 · 降门", title,
            "category=Breakthrough + effect_kind=breakthrough_drop 应合成 '突破 · 降门'，实际=" + title);
    }

    @Test
    void deriveTitle_styleCategory_returnsStyleLabel() {
        String title = InsightOfferHandler.deriveTitle("Style", "style_affinity");
        assertEquals("流派 · 亲和", title,
            "category=Style + effect_kind=style_affinity 应合成 '流派 · 亲和'，实际=" + title);
    }

    @Test
    void deriveTitle_unknownCategory_fallsBackToGanwu() {
        String title = InsightOfferHandler.deriveTitle("Unknown", "some_rate");
        assertEquals("感悟 · 通速", title,
            "未知 category 应退回 '感悟'，实际=" + title);
    }

    @Test
    void deriveTitle_nullCategory_fallsBackToGanwu() {
        String title = InsightOfferHandler.deriveTitle(null, "qi_rate");
        assertEquals("感悟 · 通速", title,
            "null category 应退回 '感悟'，实际=" + title);
    }

    @Test
    void deriveEffectSummary_withMagnitude_returnsPctFormat() {
        String summary = InsightOfferHandler.deriveEffectSummary("qi_max", 0.05);
        assertEquals("qi_max +5.0%", summary,
            "magnitude=0.05 应格式化为 '+5.0%'，实际=" + summary);
    }

    @Test
    void deriveEffectSummary_zeroMagnitude_returnsNoPct() {
        String summary = InsightOfferHandler.deriveEffectSummary("qi_max", 0.0);
        assertEquals("qi_max（无幅度）", summary,
            "magnitude=0.0 应返回 '（无幅度）' 后缀，实际=" + summary);
    }

    @Test
    void deriveEffectSummary_nullKind_returnsFallback() {
        String summary = InsightOfferHandler.deriveEffectSummary(null, 0.1);
        assertEquals("效果待结算", summary,
            "null effect_kind 应返回 '效果待结算'，实际=" + summary);
    }

    // ─── store 对 choice 字段的可观察断言 ────────────────────────────────────

    @Test
    void normalOffer_choiceCategory_matchesQi() {
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "trigger_id": "first_qi_peak",
              "choices": [
                {
                  "category": "Qi",
                  "effect_kind": "qi_recover",
                  "magnitude": 0.1,
                  "flavor_text": "真元缓缓回涌。"
                }
              ]
            }
            """;
        new InsightOfferHandler().handle(parse(json));

        InsightOfferViewModel offer = InsightOfferStore.snapshot();
        assertNotNull(offer);
        assertEquals(1, offer.choices().size(),
            "choices 大小应为 1，实际=" + offer.choices().size());
        assertEquals(InsightCategory.QI, offer.choices().get(0).category(),
            "category=Qi 应映射为 InsightCategory.QI，实际=" + offer.choices().get(0).category());
        assertEquals("insight_choice_0", offer.choices().get(0).choiceId(),
            "第 0 个 choice 的 choiceId 应为 'insight_choice_0'，实际=" + offer.choices().get(0).choiceId());
    }

    @Test
    void normalOffer_meridianCategory_populatesStore() {
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "trigger_id": "open_lung",
              "choices": [
                {
                  "category": "Meridian",
                  "effect_kind": "meridian_efficiency",
                  "magnitude": 0.08,
                  "flavor_text": "经脉通畅，气感清晰。"
                }
              ]
            }
            """;
        new InsightOfferHandler().handle(parse(json));

        InsightOfferViewModel offer = InsightOfferStore.snapshot();
        assertNotNull(offer);
        assertEquals(InsightCategory.MERIDIAN, offer.choices().get(0).category(),
            "category=Meridian 应映射为 InsightCategory.MERIDIAN，实际=" + offer.choices().get(0).category());
        // title 由 deriveTitle 合成，可断言格式
        String title = offer.choices().get(0).title();
        assertTrue(title.startsWith("经脉"),
            "Meridian 类别的 title 应以 '经脉' 开头，实际=" + title);
    }

    // ─── 含 cost_* 两态 ──────────────────────────────────────────────────────

    @Test
    void offerWithCost_effectSummaryAndCostSummaryAreSet() {
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "trigger_id": "forge_qi_pulse",
              "choices": [
                {
                  "category": "Qi",
                  "effect_kind": "qi_bonus",
                  "magnitude": 0.12,
                  "flavor_text": "气息磅礴而来。",
                  "cost_kind": "composure_tolerance",
                  "cost_magnitude": 0.03,
                  "cost_flavor": "心神微有波动。"
                }
              ]
            }
            """;
        new InsightOfferHandler().handle(parse(json));

        InsightOfferViewModel offer = InsightOfferStore.snapshot();
        assertNotNull(offer);
        String costSummary = offer.choices().get(0).costSummary();
        assertNotNull(costSummary, "costSummary 不应为 null");
        assertFalse(costSummary.isBlank(), "含 cost_kind 时 costSummary 不应为空");
        // 具体值：cost_kind=composure_tolerance, cost_magnitude=0.03 → "composure_tolerance +3.0%"
        assertEquals("composure_tolerance +3.0%", costSummary,
            "cost_kind=composure_tolerance + cost_magnitude=0.03 应合成 'composure_tolerance +3.0%'，实际=" + costSummary);
        String costFlavor = offer.choices().get(0).costFlavor();
        assertEquals("心神微有波动。", costFlavor,
            "cost_flavor 应 pass-through 到 InsightChoice.costFlavor，实际=" + costFlavor);
    }

    @Test
    void offerWithoutCost_costSummaryFallsBackToDefault() {
        // 不含 cost_kind / cost_magnitude：costSummary 退回 "代价待结算"
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "trigger_id": "calm_meditation",
              "choices": [
                {
                  "category": "Composure",
                  "effect_kind": "composure_factor",
                  "magnitude": 0.07,
                  "flavor_text": "心如止水。"
                }
              ]
            }
            """;
        new InsightOfferHandler().handle(parse(json));

        InsightOfferViewModel offer = InsightOfferStore.snapshot();
        assertNotNull(offer);
        String costSummary = offer.choices().get(0).costSummary();
        assertEquals("代价待结算", costSummary,
            "无 cost_kind 时 costSummary 应为 '代价待结算'，实际=" + costSummary);
    }

    // ─── 畸形 payload 守卫：缺 trigger_id → noOp ─────────────────────────────

    @Test
    void missingTriggerId_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "choices": [
                {
                  "category": "Qi",
                  "effect_kind": "qi_max",
                  "magnitude": 0.05,
                  "flavor_text": "气息涌来。"
                }
              ]
            }
            """;
        ServerDataDispatch dispatch = new InsightOfferHandler().handle(parse(json));

        assertFalse(dispatch.handled(),
            "缺 trigger_id 应返回 noOp（handled=false），实际=" + dispatch.handled());
        assertNull(InsightOfferStore.snapshot(),
            "缺 trigger_id 时 InsightOfferStore 不应被写入，snapshot 应为 null");
    }

    @Test
    void blankTriggerId_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "trigger_id": "   ",
              "choices": [
                {
                  "category": "Qi",
                  "effect_kind": "qi_max",
                  "magnitude": 0.05,
                  "flavor_text": "气息涌来。"
                }
              ]
            }
            """;
        // 注：trigger_id 是空格字串，但 ServerDataEnvelope 会把空格串当 string 传给 handler；
        // handler 内部检 isBlank() 应返回 noOp。
        // 注意：ServerDataEnvelope.parse 本身不会因 "   " 报错，所以 envelope 可构造。
        ServerDataEnvelope envelope = ServerDataEnvelope
            .parse(json, json.getBytes(StandardCharsets.UTF_8).length)
            .envelope();

        ServerDataDispatch dispatch = new InsightOfferHandler().handle(envelope);
        assertFalse(dispatch.handled(),
            "空白 trigger_id 应返回 noOp，实际=" + dispatch.handled());
        assertNull(InsightOfferStore.snapshot(),
            "空白 trigger_id 时 store 不应被写入");
    }

    @Test
    void emptyChoices_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "trigger_id": "valid_trigger",
              "choices": []
            }
            """;
        ServerDataDispatch dispatch = new InsightOfferHandler().handle(parse(json));

        assertFalse(dispatch.handled(),
            "choices 为空数组应返回 noOp，实际=" + dispatch.handled());
        assertNull(InsightOfferStore.snapshot(),
            "choices 为空时 store 不应被写入");
    }

    @Test
    void missingChoicesField_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "trigger_id": "valid_trigger"
            }
            """;
        ServerDataDispatch dispatch = new InsightOfferHandler().handle(parse(json));

        assertFalse(dispatch.handled(),
            "缺 choices 字段应返回 noOp，实际=" + dispatch.handled());
        assertNull(InsightOfferStore.snapshot(),
            "缺 choices 字段时 store 不应被写入");
    }

    // ─── 三个 choice 上限（plan P2 最多 3 个）────────────────────────────────

    @Test
    void threeChoices_allMappedIntoStore() {
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "trigger_id": "triple_path",
              "choices": [
                {"category":"Qi",         "effect_kind":"qi_max",       "magnitude":0.05,"flavor_text":"气海扩张。"},
                {"category":"Meridian",   "effect_kind":"meridian_rate","magnitude":0.08,"flavor_text":"经脉通畅。"},
                {"category":"Composure",  "effect_kind":"composure_recover","magnitude":0.03,"flavor_text":"心神平静。"}
              ]
            }
            """;
        new InsightOfferHandler().handle(parse(json));

        InsightOfferViewModel offer = InsightOfferStore.snapshot();
        assertNotNull(offer);
        assertEquals(3, offer.choices().size(),
            "三个 choice 应全部映射，实际 size=" + offer.choices().size());
        assertEquals("insight_choice_0", offer.choices().get(0).choiceId(),
            "第 0 个 choice id 应为 'insight_choice_0'");
        assertEquals("insight_choice_1", offer.choices().get(1).choiceId(),
            "第 1 个 choice id 应为 'insight_choice_1'");
        assertEquals("insight_choice_2", offer.choices().get(2).choiceId(),
            "第 2 个 choice id 应为 'insight_choice_2'");
    }

    // ─── choices=4 截断到 3 的 off-by-one 边界 ──────────────────────────────────

    @Test
    void fourChoices_truncatedToThree() {
        // plan §P2 choices maxItems=3；4 个 choice 时应截断，只保留前 3 个（off-by-one 守卫）
        String json = """
            {
              "v": 1,
              "type": "insight_offer",
              "trigger_id": "quad_path",
              "choices": [
                {"category":"Qi",         "effect_kind":"qi_max",            "magnitude":0.05,"flavor_text":"气海扩张。"},
                {"category":"Meridian",   "effect_kind":"meridian_rate",     "magnitude":0.08,"flavor_text":"经脉通畅。"},
                {"category":"Composure",  "effect_kind":"composure_recover", "magnitude":0.03,"flavor_text":"心神平静。"},
                {"category":"Perception", "effect_kind":"perception_bonus",  "magnitude":0.06,"flavor_text":"感知敏锐。"}
              ]
            }
            """;
        new InsightOfferHandler().handle(parse(json));

        InsightOfferViewModel offer = InsightOfferStore.snapshot();
        assertNotNull(offer, "4 个 choice 截断后 offer 不应为 null");
        assertEquals(3, offer.choices().size(),
            "choices=4 时应截断到 3（plan P2 maxItems=3），实际 size=" + offer.choices().size());
        // 确保截断的是第 4 个，前 3 个保留
        assertEquals(InsightCategory.QI,        offer.choices().get(0).category(),
            "截断后第 0 个 category 应为 QI，实际=" + offer.choices().get(0).category());
        assertEquals(InsightCategory.MERIDIAN,  offer.choices().get(1).category(),
            "截断后第 1 个 category 应为 MERIDIAN，实际=" + offer.choices().get(1).category());
        assertEquals(InsightCategory.COMPOSURE, offer.choices().get(2).category(),
            "截断后第 2 个 category 应为 COMPOSURE（Perception 应被截掉），实际=" + offer.choices().get(2).category());
    }

    @Test
    void defaultRouter_registersInsightOffer() {
        assertTrue(ServerDataRouter.createDefault().registeredTypes().contains("insight_offer"),
            "ServerDataRouter.createDefault() 应注册 'insight_offer' type");
    }

    // ─── helper ──────────────────────────────────────────────────────────────

    private static ServerDataEnvelope parse(String json) {
        return ServerDataEnvelope
            .parse(json, json.getBytes(StandardCharsets.UTF_8).length)
            .envelope();
    }
}
