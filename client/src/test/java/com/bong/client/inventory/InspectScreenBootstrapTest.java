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
                InventoryModel.SMALL_POUCH_CONTAINER_ID,
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
        InventoryModel authoritative = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(InventoryItem.simple("starter_talisman", "初始护符"), InventoryModel.PRIMARY_CONTAINER_ID, 0, 0)
            .gridItem(InventoryItem.simple("satchel_map", "挂包地图"), InventoryModel.FRONT_SATCHEL_CONTAINER_ID, 1, 2)
            .build();

        List<InventoryModel.GridEntry> mainEntries = InspectScreen.gridEntriesForContainer(
            authoritative,
            InspectScreen.containerDefAt(authoritative, 0).id()
        );
        List<InventoryModel.GridEntry> satchelEntries = InspectScreen.gridEntriesForContainer(
            authoritative,
            InspectScreen.containerDefAt(authoritative, 2).id()
        );

        assertEquals(1, mainEntries.size());
        assertEquals("starter_talisman", mainEntries.get(0).item().itemId());
        assertEquals(1, satchelEntries.size());
        assertEquals("satchel_map", satchelEntries.get(0).item().itemId());
        assertEquals(1, satchelEntries.get(0).row());
        assertEquals(2, satchelEntries.get(0).col());
    }

    @Test
    void populateContainerGridsRoutesEntriesIntoMatchingPanels() {
        InventoryModel authoritative = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(InventoryItem.simple("starter_talisman", "初始护符"), InventoryModel.PRIMARY_CONTAINER_ID, 0, 0)
            .gridItem(InventoryItem.simple("spirit_thread", "灵丝"), InventoryModel.SMALL_POUCH_CONTAINER_ID, 2, 1)
            .gridItem(InventoryItem.simple("satchel_map", "挂包地图"), InventoryModel.FRONT_SATCHEL_CONTAINER_ID, 1, 2)
            .build();

        BackpackGridPanel[] panels = new BackpackGridPanel[] {
            new BackpackGridPanel(InventoryModel.PRIMARY_CONTAINER_ID, 5, 7),
            new BackpackGridPanel(InventoryModel.SMALL_POUCH_CONTAINER_ID, 3, 3),
            new BackpackGridPanel(InventoryModel.FRONT_SATCHEL_CONTAINER_ID, 3, 4)
        };

        InspectScreen.populateContainerGrids(authoritative, panels);

        assertEquals("starter_talisman", panels[0].itemAt(0, 0).itemId());
        assertEquals("spirit_thread", panels[1].itemAt(2, 1).itemId());
        assertEquals("satchel_map", panels[2].itemAt(1, 2).itemId());
        assertNull(panels[1].itemAt(0, 0));
    }
}
