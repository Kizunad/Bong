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

    @Test
    void dispatchApplyPillSelfAlsoSendsForHuiyuanPill() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            1004L,
            "huiyuan_pill",
            "回元丹",
            1,
            1,
            0.2,
            "rare",
            "战斗中温补真元，只恢复当前真元，不提升真元上限。",
            1,
            1.0,
            1.0
        );

        assertTrue(screen.dispatchApplyPillSelf(item));
        assertEquals(1, sent.size());
        assertEquals(
            "{\"type\":\"apply_pill\",\"v\":1,\"instance_id\":1004,\"target\":{\"kind\":\"self\"}}",
            sent.get(0).body()
        );
    }

    @Test
    void forgeStationAnvilMenuIncludesPlaceAction() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            2001L,
            "ling_iron_anvil",
            "灵铁砧",
            2,
            2,
            12.0,
            "uncommon",
            "炼器砧。",
            1,
            0.8,
            1.0
        );

        assertTrue(screen.openPillContextMenu(item, 10, 20));
        assertEquals(1, screen.availablePillMenuActions(item).size());
        assertEquals("放置炼器砧", screen.availablePillMenuActions(item).get(0).label());
        assertTrue(sent.isEmpty());
    }

    @Test
    void dispatchPlaceForgeStationSendsTierFromItemId() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            2002L,
            "xuan_iron_anvil",
            "玄铁砧",
            2,
            2,
            16.0,
            "rare",
            "炼器砧。",
            1,
            0.9,
            1.0
        );

        assertTrue(screen.dispatchPlaceForgeStationAt(item, -12, 64, 38));
        assertEquals(1, sent.size());
        assertEquals(
            "{\"type\":\"forge_station_place\",\"v\":1,\"x\":-12,\"y\":64,\"z\":38,\"item_instance_id\":2002,\"station_tier\":3}",
            sent.get(0).body()
        );
    }

    @Test
    void dispatchPlaceForgeStationSkipsUnsupportedItem() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            2003L,
            "spirit_wood",
            "灵木",
            1,
            2,
            1.2,
            "common",
            "真元载体。",
            1,
            0.8,
            1.0
        );

        assertFalse(screen.dispatchPlaceForgeStationAt(item, 0, 64, 0));
        assertTrue(sent.isEmpty());
    }

    @Test
    void spiritNicheStoneMenuIncludesPlaceAction() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            3001L,
            "spirit_niche_stone",
            "龛石",
            1,
            1,
            0.4,
            "rare",
            "可埋作私有灵龛的冷石。",
            1,
            0.2,
            1.0
        );

        assertTrue(screen.openPillContextMenu(item, 10, 20));
        assertEquals(1, screen.availablePillMenuActions(item).size());
        assertEquals("放置灵龛", screen.availablePillMenuActions(item).get(0).label());
        assertTrue(sent.isEmpty());
    }

    @Test
    void dispatchPlaceSpiritNicheSendsStonePlacement() {
        install();
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        InventoryItem item = InventoryItem.createFull(
            3002L,
            "spirit_niche_stone",
            "龛石",
            1,
            1,
            0.4,
            "rare",
            "可埋作私有灵龛的冷石。",
            1,
            0.2,
            1.0
        );

        assertTrue(screen.dispatchPlaceSpiritNicheAt(item, 11, 64, 10));
        assertEquals(1, sent.size());
        assertEquals(
            "{\"type\":\"spirit_niche_place\",\"v\":1,\"x\":11,\"y\":64,\"z\":10,\"item_instance_id\":3002}",
            sent.get(0).body()
        );
    }
}
