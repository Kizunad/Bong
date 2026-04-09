package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class EventAlertStateTest {
    @BeforeEach
    void setUp() {
        EventAlertState.clear();
    }

    @AfterEach
    void tearDown() {
        EventAlertState.clear();
    }

    @Test
    public void typedEventAlertPayloadRoutesIntoBannerState() {
        BongServerPayload.EventAlertPayload payload = new BongServerPayload.EventAlertPayload(
                1,
                new BongServerPayload.EventAlert(
                        "thunder_tribulation",
                        "雷劫将至",
                        "血谷上空劫云汇聚，三十息内可能落雷。",
                        "critical",
                        "blood_valley"
                )
        );

        assertTrue(BongServerPayloadRouter.route(null, payload));

        EventAlertState.BannerState banner = EventAlertState.getCurrentBanner(1_000L);
        assertNotNull(banner);
        assertEquals("雷劫将至", banner.title());
        assertEquals("血谷上空劫云汇聚，三十息内可能落雷。", banner.detail());
        assertEquals("Blood Valley", banner.zoneLabel());
        assertEquals(EventAlertState.Severity.CRITICAL, banner.severity());
    }

    @Test
    public void eventAlertClipsLongTextAndNormalizesUnknownSeverity() {
        EventAlertState.AlertSnapshot snapshot = EventAlertState.snapshotOf(
                new BongServerPayload.EventAlert(
                        "beast_tide_warning",
                        "这是一个非常非常长的标题它会被截断避免遮挡过多画面信息",
                        "这是一个非常非常长的说明文本它会被截断以确保 overlay 仍然轻量可读并且不会把右上角全部撑满。",
                        "mystery",
                        "the_extremely_long_and_windy_blood_valley_of_echoes"
                ),
                2_000L
        );

        assertEquals("这是一个非常非常长的标题它会被截断避免遮挡...", snapshot.title());
        assertEquals("这是一个非常非常长的说明文本它会被截断以确保 overlay 仍然轻量可读并且不会把右上角全部撑满。", snapshot.detail());
        assertEquals("The Extremely Long An...", snapshot.zoneLabel());
        assertEquals(EventAlertState.Severity.INFO, snapshot.severity());
    }

    @Test
    public void bannerAlphaFadesPredictably() {
        long recordedAtMs = 1_000L;
        long expiresAtMs = recordedAtMs + EventAlertState.BANNER_DURATION_MS;

        assertEquals(0, EventAlertState.bannerAlpha(recordedAtMs, recordedAtMs, expiresAtMs));
        assertEquals(128, EventAlertState.bannerAlpha(1_125L, recordedAtMs, expiresAtMs));
        assertEquals(255, EventAlertState.bannerAlpha(2_000L, recordedAtMs, expiresAtMs));
        assertEquals(255, EventAlertState.bannerAlpha(6_500L, recordedAtMs, expiresAtMs));
        assertEquals(128, EventAlertState.bannerAlpha(6_750L, recordedAtMs, expiresAtMs));
        assertEquals(0, EventAlertState.bannerAlpha(expiresAtMs, recordedAtMs, expiresAtMs));
    }

    @Test
    public void expiredAlertReturnsNullBanner() {
        EventAlertState.recordAlert(
                new BongServerPayload.EventAlert("thunder_tribulation", "雷劫将至", "三十息内可能落雷。", "critical", "blood_valley"),
                1_000L
        );

        assertNull(EventAlertState.getCurrentBanner(7_000L));
    }
}
