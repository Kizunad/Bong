package com.bong.client.inventory;

import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.WeaponEquippedStore;
import com.bong.client.cultivation.ColorKind;
import com.bong.client.cultivation.QiColorObservedState;
import com.bong.client.cultivation.QiColorObservedStore;
import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InspectScreenBootstrapTest {

    @AfterEach
    void resetStore() {
        InventoryStateStore.resetForTests();
        WeaponEquippedStore.resetForTests();
        QiColorObservedStore.resetForTests();
    }

    @Test
    void disconnectedStateDoesNotOpenInspectScreen() {
        InventoryStateStore.clearOnDisconnect();

        InspectScreen screen = InspectScreenBootstrap.createScreenForCurrentState();

        assertNull(screen);
    }

    @Test
    void connectedLoadingStateDoesNotFallbackToMockScreen() {
        InventoryStateStore.replace(InventoryModel.builder().containers(InventoryModel.DEFAULT_CONTAINERS).build());

        InspectScreen screen = InspectScreenBootstrap.createScreenForCurrentState();

        assertNull(screen);
        assertFalse(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(0L, InventoryStateStore.revision());
    }

    @Test
    void authoritativeLoadedStateCreatesScreenFromStoreSnapshot() {
        InventoryModel authoritative = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(
                InventoryItem.simple("starter_talisman", "初始护符"),
                InventoryModel.BODY_POCKET_CONTAINER_ID,
                1,
                1
            )
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(authoritative, 12L);

        InspectScreen screen = InspectScreenBootstrap.createScreenForCurrentState();

        assertNotNull(screen);
        assertEquals(authoritative, screen.model());
    }

    @Test
    void clearInventorySnapshotAlsoClearsWeaponStore() {
        WeaponEquippedStore.putOrClear(
            "main_hand",
            new EquippedWeapon("main_hand", 1L, "iron_sword", "sword", 200.0f, 200.0f, 0)
        );
        QiColorObservedStore.replace(new QiColorObservedState(
            "offline:Observer",
            "offline:Observed",
            ColorKind.Intricate,
            null,
            false,
            false,
            1.0
        ));

        InspectScreenBootstrap.clearInventorySnapshot();

        assertNull(WeaponEquippedStore.get("main_hand"));
        assertNull(QiColorObservedStore.snapshot());
    }

    @Test
    void crosshairEntityTargetIsNullWithoutClientContext() {
        assertNull(InspectScreenBootstrap.crosshairEntityTarget(null));
    }

    @Test
    void multiContainerRoutingIncludesNonPrimaryTabs() {
        // Explicitly supply 3-container layout (body_pocket + back_pack + waist_pouch) to test routing.
        List<InventoryModel.ContainerDef> threeDefs = List.of(
            new InventoryModel.ContainerDef(InventoryModel.BODY_POCKET_CONTAINER_ID, "贴身口袋", 2, 3),
            new InventoryModel.ContainerDef(InventoryModel.BACK_PACK_CONTAINER_ID, "破草包", 3, 3),
            new InventoryModel.ContainerDef("waist_pouch", "腰包", 2, 2)
        );
        InventoryModel authoritative = InventoryModel.builder()
            .containers(threeDefs)
            .gridItem(InventoryItem.simple("starter_talisman", "初始护符"), InventoryModel.BODY_POCKET_CONTAINER_ID, 0, 0)
            .gridItem(InventoryItem.simple("satchel_map", "挂包地图"), "waist_pouch", 1, 1)
            .build();

        List<InventoryModel.GridEntry> pocketEntries = InspectScreen.gridEntriesForContainer(
            authoritative,
            InspectScreen.containerDefAt(authoritative, 0).id()
        );
        List<InventoryModel.GridEntry> waistEntries = InspectScreen.gridEntriesForContainer(
            authoritative,
            InspectScreen.containerDefAt(authoritative, 2).id()
        );

        assertEquals(1, pocketEntries.size());
        assertEquals("starter_talisman", pocketEntries.get(0).item().itemId());
        assertEquals(1, waistEntries.size());
        assertEquals("satchel_map", waistEntries.get(0).item().itemId());
        assertEquals(1, waistEntries.get(0).row());
        assertEquals(1, waistEntries.get(0).col());
    }

    @Test
    void populateContainerGridsRoutesEntriesIntoMatchingPanels() {
        InventoryModel authoritative = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(InventoryItem.simple("starter_talisman", "初始护符"), InventoryModel.BODY_POCKET_CONTAINER_ID, 0, 0)
            .gridItem(InventoryItem.simple("spirit_grass", "灵草"), InventoryModel.BACK_PACK_CONTAINER_ID, 2, 1)
            .build();

        BackpackGridPanel[] panels = new BackpackGridPanel[] {
            new BackpackGridPanel(InventoryModel.BODY_POCKET_CONTAINER_ID, 2, 3),
            new BackpackGridPanel(InventoryModel.BACK_PACK_CONTAINER_ID, 3, 3)
        };

        InspectScreen.populateContainerGrids(authoritative, panels);

        assertEquals("starter_talisman", panels[0].itemAt(0, 0).itemId());
        assertEquals("spirit_grass", panels[1].itemAt(2, 1).itemId());
        assertNull(panels[0].itemAt(1, 1));
    }
}
