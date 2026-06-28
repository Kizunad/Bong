package com.bong.client.inventory;

import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.InventoryModel.ContainerDef;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.inventory.state.DragState;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * B 准则（快捷栏来源限制）—— {@link InspectScreen#isQuickAccessSource} 来源门控对拍。
 *
 * <p>放行：QUICK_USE（栏内重排）/ GRID body_pocket / GRID quickAccess=true 容器（[快捷]背包）。
 * 拒绝：GRID quickAccess=false 普通背包 / GRID 未知容器 / GRID 来源 id 为 null /
 * HOTBAR / EQUIP / MERIDIAN / BODY_PART。HOTBAR 按默认决策拒绝（DragState 不记 sourceContainerId）。</p>
 */
public class InspectScreenQuickAccessSourceTest {

    private static InventoryItem item() {
        return InventoryItem.createFull(
            500L, "spirit_herb", "灵草", 1, 1, 0.1, "common", "", 1, 1.0, 1.0);
    }

    /** 含 body_pocket(true) + 快捷背包 pack_quick(true) + 普通背包 pack_plain(false) 的屏。 */
    private static InspectScreen screen() {
        InventoryModel model = InventoryModel.builder()
            .containers(List.of(
                new ContainerDef(InventoryModel.BODY_POCKET_CONTAINER_ID, "贴身口袋", 2, 3, null, true),
                new ContainerDef("pack_quick", "快捷腰包", 2, 2, 700L, true),
                new ContainerDef("pack_plain", "破草包", 3, 3, 800L, false)))
            .build();
        return new InspectScreen(model);
    }

    private static DragState gridFrom(String containerId) {
        DragState ds = new DragState();
        ds.pickup(item(), containerId, 0, 0);
        return ds;
    }

    @Test
    void bodyPocketSourceAllowed() {
        assertTrue(screen().isQuickAccessSource(gridFrom(InventoryModel.BODY_POCKET_CONTAINER_ID)),
            "body_pocket 内物品应放行指派到快捷栏");
    }

    @Test
    void quickAccessPackSourceAllowed() {
        assertTrue(screen().isQuickAccessSource(gridFrom("pack_quick")),
            "quickAccess=true 的 [快捷] 背包内物品应放行");
    }

    @Test
    void plainPackSourceRejected() {
        assertFalse(screen().isQuickAccessSource(gridFrom("pack_plain")),
            "quickAccess=false 的普通背包内物品应拒绝（不可入快捷栏）");
    }

    @Test
    void unknownContainerSourceRejected() {
        assertFalse(screen().isQuickAccessSource(gridFrom("pack_ghost")),
            "未知容器 id（不在 model.containers()）应拒绝（兜底保守）");
    }

    @Test
    void nullSourceContainerRejected() {
        DragState ds = new DragState();
        ds.pickup(item(), 0, 0); // 2 参 pickup → sourceContainerId == null
        assertFalse(screen().isQuickAccessSource(ds),
            "GRID 来源容器 id 为 null（异常）应拒绝");
    }

    @Test
    void quickUseReorderAllowed() {
        DragState ds = new DragState();
        ds.pickupFromQuickUse(item(), 0);
        assertTrue(screen().isQuickAccessSource(ds),
            "QUICK_USE 来源（快捷栏内重排）应放行");
    }

    @Test
    void hotbarSourceRejectedByDefault() {
        DragState ds = new DragState();
        ds.pickupFromHotbar(item(), 0);
        assertFalse(screen().isQuickAccessSource(ds),
            "HOTBAR 来源默认拒绝（主手栏不属任何容器，DragState 不记 sourceContainerId）");
    }

    @Test
    void equipSourceRejected() {
        DragState ds = new DragState();
        ds.pickupFromEquip(item(), EquipSlotType.CHEST);
        assertFalse(screen().isQuickAccessSource(ds), "EQUIP 来源应拒绝");
    }

    @Test
    void meridianSourceRejected() {
        DragState ds = new DragState();
        ds.pickupFromMeridian(item(), MeridianChannel.LU);
        assertFalse(screen().isQuickAccessSource(ds), "MERIDIAN 来源应拒绝");
    }

    @Test
    void bodyPartSourceRejected() {
        DragState ds = new DragState();
        ds.pickupFromBodyPart(item(), BodyPart.HEAD);
        assertFalse(screen().isQuickAccessSource(ds), "BODY_PART 来源应拒绝");
    }

    @Test
    void nullDragStateRejected() {
        assertFalse(screen().isQuickAccessSource(new DragState()),
            "未起拖（sourceKind==null）应拒绝");
    }
}
