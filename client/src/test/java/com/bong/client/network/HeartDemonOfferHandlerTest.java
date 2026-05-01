package com.bong.client.network;

import com.bong.client.insight.InsightCategory;
import com.bong.client.insight.InsightOfferStore;
import com.bong.client.insight.InsightOfferViewModel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class HeartDemonOfferHandlerTest {
    @AfterEach
    void resetStore() {
        InsightOfferStore.resetForTests();
    }

    @Test
    void handlerMapsOfferIntoInsightStore() {
        String json = """
            {
              "v": 1,
              "type": "heart_demon_offer",
              "offer_id": "heart_demon:1:1000",
              "trigger_id": "heart_demon:1:1000",
              "trigger_label": "心魔劫临身",
              "realm_label": "渡虚劫 · 心魔",
              "composure": 0.75,
              "quota_remaining": 1,
              "quota_total": 1,
              "expires_at_ms": 4102444800000,
              "choices": [
                {
                  "choice_id": "heart_demon_choice_0",
                  "category": "Composure",
                  "title": "守本心",
                  "effect_summary": "回复少量当前真元",
                  "flavor": "你把呼吸压回丹田。",
                  "style_hint": "稳妥"
                }
              ]
            }
            """;
        ServerDataEnvelope envelope = ServerDataEnvelope
            .parse(json, json.getBytes(StandardCharsets.UTF_8).length)
            .envelope();

        ServerDataDispatch dispatch = new HeartDemonOfferHandler().handle(envelope);

        assertTrue(dispatch.handled());
        InsightOfferViewModel offer = InsightOfferStore.snapshot();
        assertNotNull(offer);
        assertEquals("heart_demon:1:1000", offer.triggerId());
        assertEquals("心魔劫临身", offer.triggerLabel());
        assertEquals("渡虚劫 · 心魔", offer.realmLabel());
        assertEquals(0.75d, offer.composure());
        assertEquals(1, offer.choices().size());
        assertEquals(InsightCategory.COMPOSURE, offer.choices().get(0).category());
        assertEquals("守本心", offer.choices().get(0).title());
    }

    @Test
    void defaultRouterRegistersHeartDemonOffer() {
        assertTrue(ServerDataRouter.createDefault().registeredTypes().contains("heart_demon_offer"));
    }
}
