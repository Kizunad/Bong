package com.bong.client.combat.screen;

import com.bong.client.combat.ForgeCarrierIntent;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

class ForgeCarrierScreenTest {
    @Test
    void constructorNormalizesUnknownItemAndQiBoundaries() {
        ForgeCarrierScreen screen = new ForgeCarrierScreen(
            "unknown", 2.0, ignored -> UiIntentResult.accepted());

        assertEquals("dagger", screen.selectedItemForTests());
        assertEquals(1.0, screen.qiInvestForTests());

        screen.selectItemForTests("needle");
        screen.setQiInvestForTests(-1.0);
        assertEquals("needle", screen.selectedItemForTests());
        assertEquals(0.0, screen.qiInvestForTests());
    }

    @Test
    void submitDispatchesCurrentTypedState() {
        ForgeCarrierIntent[] captured = {null};
        UiIntentSink<ForgeCarrierIntent> sink = intent -> {
            captured[0] = intent;
            return UiIntentResult.accepted();
        };
        ForgeCarrierScreen screen = new ForgeCarrierScreen("dagger", 0.5, sink);

        screen.selectItemForTests("needle");
        screen.setQiInvestForTests(0.25);
        screen.dispatch();

        assertEquals(new ForgeCarrierIntent.Begin("needle", 0.25), captured[0]);
        assertEquals("", screen.feedbackTextForTests(), "本地提交成功不应留下旧反馈");
    }

    @Test
    void rejectedSubmitKeepsReasonForPlayer() {
        ForgeCarrierScreen screen = new ForgeCarrierScreen(
            "dagger", 0.5, ignored -> UiIntentResult.rejected("真元不足"));

        screen.dispatch();

        assertEquals("注入未提交: 真元不足", screen.feedbackTextForTests());
    }

    @Test
    void nullSinkIsRejectedAtConstruction() {
        assertThrows(NullPointerException.class,
            () -> new ForgeCarrierScreen("dagger", 0.5, null));
    }
}
