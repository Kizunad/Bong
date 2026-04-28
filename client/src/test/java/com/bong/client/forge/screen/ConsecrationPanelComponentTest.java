package com.bong.client.forge.screen;

import com.bong.client.cultivation.ColorKind;
import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class ConsecrationPanelComponentTest {
    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        ForgeSessionStore.resetForTests();
    }

    private void install() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void progress_bar_reflects_qi_ratio() {
        var state = ConsecrationPanelComponent.renderStateFrom(snapshot(
            "{\"step\":\"consecration\",\"qi_injected\":25.0,\"qi_required\":100.0}"
        ), "Spirit");

        assertEquals(0.25, state.progressRatio(), 0.0001);
        assertEquals("25 / 100", state.qiLabel());
    }

    @Test
    void color_swatch_matches_caster_color() {
        var state = ConsecrationPanelComponent.renderStateFrom(snapshot(
            "{\"step\":\"consecration\",\"qi_injected\":0.0,\"qi_required\":80.0,\"color_imprint\":\"Sharp\"}"
        ), "Spirit");

        assertEquals(ColorKind.Sharp, state.color());
        assertEquals(0xFFE8F2FF, state.color().argb());
    }

    @Test
    void inject_button_disabled_when_realm_insufficient() {
        var state = ConsecrationPanelComponent.renderStateFrom(snapshot(
            "{\"step\":\"consecration\",\"qi_injected\":0.0,\"qi_required\":80.0,\"min_realm\":\"Spirit\"}"
        ), "Condense");

        assertFalse(state.realmAllowed());
        assertFalse(state.canInject());
    }

    @Test
    void inject_button_held_emits_periodic_request() {
        install();
        ForgeSessionStore.replace(snapshot(
            "{\"step\":\"consecration\",\"qi_injected\":0.0,\"qi_required\":80.0}"
        ));
        ConsecrationPanelComponent panel = new ConsecrationPanelComponent();

        assertTrue(panel.beginInject());
        assertEquals(1, panel.tickInject(10L));
        assertEquals(0, panel.tickInject(10L));
        assertEquals(1, panel.tickInject(11L));
        assertEquals(2, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"forge_consecration_inject\",\"v\":1,\"session_id\":7,\"qi_amount\":2.5}",
            sent.get(0).body()
        );
    }

    @Test
    void progress_full_emits_no_more_inject() {
        install();
        ForgeSessionStore.replace(snapshot(
            "{\"step\":\"consecration\",\"qi_injected\":80.0,\"qi_required\":80.0}"
        ));
        ConsecrationPanelComponent panel = new ConsecrationPanelComponent();

        assertFalse(panel.beginInject());
        assertEquals(0, panel.tickInject(1L));
        assertTrue(sent.isEmpty());
    }

    private static ForgeSessionStore.Snapshot snapshot(String stepStateJson) {
        return new ForgeSessionStore.Snapshot(
            7,
            "ling_feng_v0",
            "灵锋剑",
            true,
            "consecration",
            3,
            3,
            stepStateJson
        );
    }
}
