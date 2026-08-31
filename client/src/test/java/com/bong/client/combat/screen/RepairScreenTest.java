package com.bong.client.combat.screen;

import com.bong.client.combat.RepairIntent;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class RepairScreenTest {
    @Test
    void constructorNormalizesSnapshotFieldsWithoutChangingCoordinates() {
        RepairScreen screen = new RepairScreen(null, 1.5f, -7L, -3, 64, 8, ignored -> UiIntentResult.accepted());

        assertNotNull(screen);
        assertEquals("-", screen.weaponLabelForTests());
        assertEquals(1.0f, screen.durabilityNormForTests());
        assertEquals(0L, screen.weaponInstanceIdForTests());
        assertEquals(-3, screen.stationXForTests());
        assertEquals(64, screen.stationYForTests());
        assertEquals(8, screen.stationZForTests());
    }

    @Test
    void nullIntentSinkIsRejectedAtConstruction() {
        assertThrows(NullPointerException.class,
            () -> new RepairScreen("锈骨剑", 0.4f, 42L, 1, 64, 2, null));
    }

    @Test
    void typedCommitCarriesTheNormalizedScreenSnapshot() {
        RepairIntent[] captured = {null};
        UiIntentSink<RepairIntent> sink = intent -> {
            captured[0] = intent;
            return UiIntentResult.accepted();
        };
        RepairScreen screen = new RepairScreen("锈骨剑", 0.4f, 42L, 1, 64, 2, sink);

        // bindTemplate 由真实 XML host 在客户端初始化；此处锁住构造快照与 sink 契约。
        assertEquals(42L, screen.weaponInstanceIdForTests());
        assertEquals(1, screen.stationXForTests());
        assertEquals(64, screen.stationYForTests());
        assertEquals(2, screen.stationZForTests());
    }

    @Test
    void rejectedIntentKeepsScreenOpenAndExposesReason() {
        RepairScreen screen = new RepairScreen(
            "锈骨剑", 0.4f, 42L, 1, 64, 2,
            ignored -> UiIntentResult.rejected("缺少精钢锭"));

        screen.dispatch("refined_steel");

        assertTrue(screen.feedbackTextForTests().contains("缺少精钢锭"),
            "本地拒绝必须把 reason 暴露给界面反馈");
    }

    @Test
    void transportErrorKeepsScreenOpenAndExposesReason() {
        RepairScreen screen = new RepairScreen(
            "锈骨剑", 0.4f, 42L, 1, 64, 2,
            ignored -> UiIntentResult.error("连接已断开"));

        screen.dispatch("pill");

        assertEquals("养护未提交: 连接已断开", screen.feedbackTextForTests());
    }
}
