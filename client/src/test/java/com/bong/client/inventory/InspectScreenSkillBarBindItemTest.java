package com.bong.client.inventory;

import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
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

class InspectScreenSkillBarBindItemTest {
    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        SkillBarStore.resetForTests();
    }

    @Test
    void onlyBlockItemsOpenSkillBarBindMenu() {
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        assertTrue(screen.openSkillBarContextMenu(blockItem(), 10, 20));
        assertTrue(screen.hasOpenSkillBarContextMenu());
        assertFalse(screen.openSkillBarContextMenu(
            InventoryItem.createFull(2L, "guyuan_pill", "固元丹", 1, 1, 0.2, "rare", "", 1, 1.0, 1.0),
            10,
            20
        ));
    }

    @Test
    void bindBlockItemSendsRequestAndUpdatesLocalSkillBar() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        assertTrue(screen.bindBlockItemToSkillBar(2, blockItem()));

        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"skill_bar_bind\",\"v\":1,\"slot\":2,\"binding\":{\"kind\":\"item\",\"template_id\":\"earth_crumb\"}}",
            sent.get(0).body()
        );
        SkillBarEntry entry = SkillBarStore.snapshot().slot(2);
        assertEquals(SkillBarEntry.Kind.ITEM, entry.kind());
        assertEquals("earth_crumb", entry.id());
        assertTrue(entry.iconTexture().endsWith("textures/gui/items/earth_crumb.png"));
    }

    private static InventoryItem blockItem() {
        return InventoryItem.createFull(
            1L,
            "earth_crumb",
            "土块",
            1,
            1,
            0.1,
            "common",
            "可放置的土块",
            16,
            1.0,
            1.0
        );
    }
}
