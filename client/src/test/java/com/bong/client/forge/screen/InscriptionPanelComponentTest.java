package com.bong.client.forge.screen;

import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.inventory.model.InventoryItem;
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

public class InscriptionPanelComponentTest {
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
    void renders_three_slots_for_double_handed_weapon() {
        var state = InscriptionPanelComponent.renderStateFrom(snapshot(
            "{\"step\":\"inscription\",\"slots\":[null,null,null],\"filled_slots\":0,\"failed\":false}"
        ));

        assertEquals(3, state.maxSlots());
        assertEquals(0, state.filledCount());
    }

    @Test
    void renders_one_slot_for_sword() {
        var state = InscriptionPanelComponent.renderStateFrom(snapshot(
            "{\"step\":\"inscription\",\"max_slots\":1,\"filled_slots\":0,\"failed\":false}"
        ));

        assertEquals(1, state.maxSlots());
    }

    @Test
    void accepts_inscription_scroll_drop() {
        install();
        ForgeSessionStore.replace(snapshot(
            "{\"step\":\"inscription\",\"max_slots\":1,\"filled_slots\":0,\"failed\":false}"
        ));
        InscriptionPanelComponent panel = new InscriptionPanelComponent();

        assertTrue(panel.tryDropScroll(scroll("inscription_scroll_sharp_v0")));
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"forge_inscription_scroll\",\"v\":1,\"session_id\":7,\"inscription_id\":\"sharp_v0\"}",
            sent.get(0).body()
        );
    }

    @Test
    void optimistic_duplicate_scroll_drops_count_each_slot() {
        install();
        ForgeSessionStore.replace(snapshot(
            "{\"step\":\"inscription\",\"max_slots\":2,\"filled_slots\":0,\"failed\":false}"
        ));
        InscriptionPanelComponent panel = new InscriptionPanelComponent();

        assertTrue(panel.tryDropScroll(scroll("inscription_scroll_sharp_v0")));
        assertTrue(panel.tryDropScroll(scroll("inscription_scroll_sharp_v0")));
        assertFalse(panel.tryDropScroll(scroll("inscription_scroll_sharp_v0")));

        assertEquals(2, sent.size());
        assertEquals(2, panel.currentRenderState().filledCount());
        assertEquals("sharp_v0", panel.currentRenderState().inscriptionAt(0));
        assertEquals("sharp_v0", panel.currentRenderState().inscriptionAt(1));
    }

    @Test
    void rejects_non_scroll_item_drop() {
        install();
        ForgeSessionStore.replace(snapshot(
            "{\"step\":\"inscription\",\"max_slots\":1,\"filled_slots\":0,\"failed\":false}"
        ));
        InscriptionPanelComponent panel = new InscriptionPanelComponent();

        assertFalse(panel.tryDropScroll(scroll("fan_iron_ingot")));
        assertTrue(sent.isEmpty());
    }

    @Test
    void slot_filled_state_shows_inscription_id() {
        var state = InscriptionPanelComponent.renderStateFrom(snapshot(
            "{\"step\":\"inscription\",\"slots\":[{\"inscription_id\":\"sharp_v0\"},null],\"failed\":false}"
        ));

        assertTrue(state.isSlotFilled(0));
        assertEquals("sharp_v0", state.inscriptionAt(0));
        assertFalse(state.isSlotFilled(1));
    }

    @Test
    void fail_chance_displayed_correctly() {
        var state = InscriptionPanelComponent.renderStateFrom(snapshot(
            "{\"step\":\"inscription\",\"max_slots\":1,\"filled_slots\":0,\"failed\":false,\"fail_chance_remaining\":0.25}"
        ));

        assertEquals("失败率 25%", state.failChanceLabel());
    }

    private static ForgeSessionStore.Snapshot snapshot(String stepStateJson) {
        return new ForgeSessionStore.Snapshot(
            7,
            "qing_feng_v0",
            "青锋剑",
            true,
            "inscription",
            2,
            2,
            stepStateJson
        );
    }

    private static InventoryItem scroll(String itemId) {
        return InventoryItem.createFull(
            7001L,
            itemId,
            itemId,
            1,
            1,
            0.1,
            "uncommon",
            "铭文残卷",
            1,
            1.0,
            1.0
        );
    }
}
