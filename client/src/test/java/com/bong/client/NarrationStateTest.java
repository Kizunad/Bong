package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class NarrationStateTest {
    @BeforeEach
    void setUp() {
        NarrationState.clear();
    }

    @AfterEach
    void tearDown() {
        NarrationState.clear();
    }

    @Test
    public void typedNarrationPayloadRoutesIntoNarrationState() {
        BongServerPayload.NarrationPayload payload = new BongServerPayload.NarrationPayload(
                1,
                List.of(new BongServerPayload.Narration("broadcast", "天道震怒，血谷上空劫云翻涌。", "system_warning", null))
        );

        assertTrue(BongServerPayloadRouter.route(null, payload));

        NarrationState.NarrationSnapshot latest = NarrationState.getLatestNarration();
        assertNotNull(latest);
        assertEquals("system_warning", latest.style());
        assertEquals("[天道警示] 天道震怒，血谷上空劫云翻涌。", latest.chatLine());
        assertEquals(5_000L, latest.expiresAtMs() - latest.recordedAtMs());
    }

    @Test
    public void systemWarningCreatesChatMappingAndToastState() {
        List<NarrationState.NarrationSnapshot> chatEvents = new ArrayList<>();
        BongServerPayload.Narration narration = new BongServerPayload.Narration(
                "broadcast",
                "雷劫将至，速避高处。",
                "system_warning",
                null
        );

        NarrationState.NarrationSnapshot snapshot = NarrationState.recordNarration(narration, 1_000L, chatEvents::add);

        assertEquals(List.of(snapshot), chatEvents);
        assertEquals("[天道警示] 雷劫将至，速避高处。", snapshot.chatLine());
        assertEquals("system_warning", NarrationState.getLatestNarration().style());
        assertEquals(6_000L, NarrationState.getLatestNarration().expiresAtMs());

        NarrationState.ToastState toast = NarrationState.getCurrentToast(5_999L);
        assertNotNull(toast);
        assertEquals(snapshot.chatLine(), toast.text());
        assertEquals(0xFF5555, toast.color());
        assertEquals(6_000L, toast.expiresAtMs());
        assertNull(NarrationState.getCurrentToast(6_000L));
    }

    @Test
    public void eraDecreeUsesGoldToastDuration() {
        BongServerPayload.Narration narration = new BongServerPayload.Narration(
                "broadcast",
                "天地新纪已开，万宗共听敕令。",
                "era_decree",
                null
        );

        NarrationState.NarrationSnapshot snapshot = NarrationState.recordNarration(narration, 2_000L, ignored -> {
        });

        assertEquals("[时代宣令] 天地新纪已开，万宗共听敕令。", snapshot.chatLine());
        assertEquals(10_000L, snapshot.expiresAtMs());

        NarrationState.ToastState toast = NarrationState.getCurrentToast(9_999L);
        assertNotNull(toast);
        assertEquals(0xFFD700, toast.color());
        assertEquals(10_000L, toast.expiresAtMs());
        assertNull(NarrationState.getCurrentToast(10_000L));
    }

    @Test
    public void ordinaryNarrationOnlyEnqueuesChatLine() {
        List<NarrationState.NarrationSnapshot> chatEvents = new ArrayList<>();
        BongServerPayload.Narration narration = new BongServerPayload.Narration(
                "broadcast",
                "山风拂过，古木间响起细碎回音。",
                "narration",
                null
        );

        NarrationState.NarrationSnapshot snapshot = NarrationState.recordNarration(narration, 3_000L, chatEvents::add);

        assertEquals(List.of(snapshot), chatEvents);
        assertEquals("[叙事] 山风拂过，古木间响起细碎回音。", snapshot.chatLine());
        assertEquals(0L, snapshot.expiresAtMs());
        assertNull(NarrationState.getCurrentToast(3_001L));
    }
}
