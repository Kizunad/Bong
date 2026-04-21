package com.bong.client.inventory;

import com.bong.client.inventory.component.BodyInspectComponent;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.MeridianChannel;
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

public class InspectScreenApplyPillTest {

    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
    }

    private void install() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    @Test
    void dispatchApplyPillSelfSendsForAuthoritativeGuyuanPill() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1001L,
            "guyuan_pill",
            "固元丹",
            1,
            1,
            0.2,
            "rare",
            "温补真元，服后可加速恢复灵力。",
            1,
            1.0,
            1.0
        );

        assertTrue(screen.dispatchApplyPillSelf(item));
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"apply_pill\",\"v\":1,\"instance_id\":1001,\"target\":{\"kind\":\"self\"}}",
            sent.get(0).body()
        );
    }

    @Test
    void openPillContextMenuForGuyuanPillDoesNotSendImmediately() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1001L, "guyuan_pill", "固元丹", 1, 1, 0.2, "rare",
            "温补真元，服后可加速恢复灵力。", 1, 1.0, 1.0
        );

        assertTrue(screen.openPillContextMenu(item, 10, 20));
        assertTrue(screen.hasOpenPillContextMenu());
        assertTrue(sent.isEmpty());
        assertEquals(1, screen.availablePillMenuActions(item).size());
        assertEquals("服用", screen.availablePillMenuActions(item).get(0).label());
    }

    @Test
    void dispatchApplyPillSelfSkipsMockItemsWithoutAuthoritativeInstanceId() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.create(
            "guyuan_pill",
            "固元丹",
            1,
            1,
            0.2,
            "rare",
            "温补真元，服后可加速恢复灵力。"
        );

        assertFalse(screen.dispatchApplyPillSelf(item));
        assertTrue(sent.isEmpty());
    }

    @Test
    void dispatchApplyPillSelfSkipsUnsupportedItems() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1002L,
            "ningmai_powder",
            "凝脉散",
            1,
            1,
            0.3,
            "uncommon",
            "外敷经脉，缓解走火入魔。",
            1,
            1.0,
            1.0
        );

        assertFalse(screen.dispatchApplyPillSelf(item));
        assertTrue(sent.isEmpty());
    }

    @Test
    void dispatchApplyPillMeridianSendsForSelectedChannelAndNingmaiPowder() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        BodyInspectComponent bodyInspect = new BodyInspectComponent();
        bodyInspect.setSelectedChannel(MeridianChannel.LU);
        screen.setBodyInspectForTests(bodyInspect);
        InventoryItem item = InventoryItem.createFull(
            1002L,
            "ningmai_powder",
            "凝脉散",
            1,
            1,
            0.3,
            "uncommon",
            "外敷经脉，缓解走火入魔。",
            1,
            1.0,
            1.0
        );

        assertTrue(screen.dispatchApplyPillMeridian(item));
        assertEquals(1, sent.size());
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals(
            "{\"type\":\"apply_pill\",\"v\":1,\"instance_id\":1002,\"target\":{\"kind\":\"meridian\",\"meridian_id\":\"Lung\"}}",
            sent.get(0).body()
        );
    }

    @Test
    void meridianPillMenuActionEntersPendingTargetModeBeforeSending() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1002L, "ningmai_powder", "凝脉散", 1, 1, 0.3, "uncommon",
            "外敷经脉，缓解走火入魔。", 1, 1.0, 1.0
        );

        assertTrue(screen.openPillContextMenu(item, 10, 20));
        assertEquals(1, screen.availablePillMenuActions(item).size());
        assertEquals("外敷（选经脉）", screen.availablePillMenuActions(item).get(0).label());

        screen.triggerPillMenuAction(InspectScreen.ActionKind.MERIDIAN_TARGET);

        assertFalse(screen.hasOpenPillContextMenu());
        assertTrue(screen.hasPendingMeridianUse());
        assertTrue(sent.isEmpty());
    }

    @Test
    void confirmPendingMeridianUseSendsUsingFocusedChannel() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        BodyInspectComponent bodyInspect = new BodyInspectComponent();
        bodyInspect.setSelectedChannel(MeridianChannel.LU);
        screen.setBodyInspectForTests(bodyInspect);
        InventoryItem item = InventoryItem.createFull(
            1002L, "ningmai_powder", "凝脉散", 1, 1, 0.3, "uncommon",
            "外敷经脉，缓解走火入魔。", 1, 1.0, 1.0
        );

        assertTrue(screen.openPillContextMenu(item, 10, 20));
        screen.triggerPillMenuAction(InspectScreen.ActionKind.MERIDIAN_TARGET);

        assertTrue(screen.confirmPendingMeridianUse());
        assertFalse(screen.hasPendingMeridianUse());
        assertEquals(1, sent.size());
        assertEquals(
            "{\"type\":\"apply_pill\",\"v\":1,\"instance_id\":1002,\"target\":{\"kind\":\"meridian\",\"meridian_id\":\"Lung\"}}",
            sent.get(0).body()
        );
    }

    @Test
    void dispatchApplyPillSelfAlsoSendsForForbiddenHuiyuanPill() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1003L,
            "huiyuan_pill_forbidden",
            "回元丹·禁药",
            1,
            1,
            0.2,
            "legendary",
            "禁药版回元丹，可瞬间排尽异种真元，然代价为反噬经脉。",
            1,
            1.0,
            1.0
        );

        assertTrue(screen.dispatchApplyPillSelf(item));
        assertEquals(1, sent.size());
        assertEquals(
            "{\"type\":\"apply_pill\",\"v\":1,\"instance_id\":1003,\"target\":{\"kind\":\"self\"}}",
            sent.get(0).body()
        );
    }
}
