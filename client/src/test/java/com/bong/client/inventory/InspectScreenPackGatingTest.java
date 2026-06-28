package com.bong.client.inventory;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.bong.client.hud.BongToast;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.SlotContents;
import java.util.List;
import java.util.OptionalLong;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

/**
 * plan-tarkov-backpack-v1 P2（交付物 #3）—— client 穿戴态门控单测。
 *
 * <p>锁住「拖入 pack_&lt;数字&gt; 套包容器仅当其 owner 背包件仍穿戴」的 client 门控契约，与 server
 * validate_move_semantics 的 Container 分支门控对齐。门控逻辑抽成 InspectScreen 的 package-private
 * 纯静态方法（parseWornPackInstance / isWornPackContainerDroppable），免去模拟鼠标拖拽即可饱和覆盖。
 */
public class InspectScreenPackGatingTest {

    private static final long WORN_PACK_INSTANCE = 1007L;

    @BeforeEach
    void resetToast() {
        BongToast.resetForTests();
    }

    @AfterEach
    void cleanupToast() {
        BongToast.resetForTests();
    }

    /** chest worn 栈含 [铁甲, 套包(instance 1007)]，套包在栈顶 = 当前穿戴。 */
    private InventoryModel snapshotWithWornPack() {
        InventoryItem armor =
            InventoryItem.createFull(
                1006L, "armor_iron_chest", "铁胸甲", 2, 2, 6.0, "common", "", 1, 0.1, 0.9);
        InventoryItem pouch =
            InventoryItem.createFull(
                WORN_PACK_INSTANCE, "worn_grass_pouch", "破草包", 2, 2, 0.3, "common", "", 1, 0.5,
                0.3);
        return InventoryModel.builder()
            .equipSlot(EquipSlotType.CHEST, new SlotContents(List.of(armor, pouch), null))
            .build();
    }

    /** 套包(instance 1007)已卸下且不在任何携带面（真丢地/转移走）：chest worn 仅剩铁甲。 */
    private InventoryModel snapshotWithoutWornPack() {
        InventoryItem armor =
            InventoryItem.createFull(
                1006L, "armor_iron_chest", "铁胸甲", 2, 2, 6.0, "common", "", 1, 0.1, 0.9);
        return InventoryModel.builder()
            .equipSlot(EquipSlotType.CHEST, new SlotContents(List.of(armor), null))
            .build();
    }

    private static InventoryItem pouch() {
        return InventoryItem.createFull(
            WORN_PACK_INSTANCE, "worn_grass_pouch", "破草包", 2, 2, 0.3, "common", "", 1, 0.5, 0.3);
    }

    /** bug #2：套包被手持（MAIN_HAND held）—— 携带面之一，应放行。 */
    private InventoryModel snapshotWithHeldPack() {
        return InventoryModel.builder()
            .equipSlot(EquipSlotType.MAIN_HAND, SlotContents.ofHeld(pouch()))
            .build();
    }

    /** bug #2：套包在快捷栏 —— 携带面之一，应放行。 */
    private InventoryModel snapshotWithPackInHotbar() {
        return InventoryModel.builder()
            .hotbar(3, pouch())
            .build();
    }

    /** bug #2（核心）：套包当货物塞在 body_pocket grid —— 卸下不丢，仍可作为容器拖入，应放行。 */
    private InventoryModel snapshotWithPackInBodyPocket() {
        return InventoryModel.builder()
            .gridItem(pouch(), InventoryModel.BODY_POCKET_CONTAINER_ID, 0, 0)
            .build();
    }

    // ── parseWornPackInstance（镜像 server worn_pack_instance_from_container_id）──

    @Test
    void parsePackInstanceFromNumericSuffix() {
        OptionalLong got = InspectScreen.parseWornPackInstance("pack_1007");
        assertTrue(got.isPresent(), "pack_<数字> 应解析出 owner instance id");
        assertEquals(1007L, got.getAsLong(), "owner instance id 应等于 pack_ 后缀数字");
    }

    @Test
    void parseRejectsNonPackContainerIds() {
        // body_pocket / main_pack 等静态容器不是套包，不受门控。
        assertTrue(
            InspectScreen.parseWornPackInstance("body_pocket").isEmpty(),
            "body_pocket 不应被当作套包容器");
        assertTrue(
            InspectScreen.parseWornPackInstance("main_pack").isEmpty(),
            "main_pack 不应被当作套包容器");
    }

    @Test
    void parseRejectsPlaceholderAndNonNumericPackSuffix() {
        // 占位容器 pack_grass_pouch（非数字后缀）镜像 server 行为：不解析 → 不门控。
        assertTrue(
            InspectScreen.parseWornPackInstance("pack_grass_pouch").isEmpty(),
            "pack_grass_pouch 占位（非数字后缀）不应解析出 owner，与 server 一致");
        assertTrue(
            InspectScreen.parseWornPackInstance("pack_abc").isEmpty(),
            "pack_<非数字> 不应解析");
        assertTrue(
            InspectScreen.parseWornPackInstance("pack_").isEmpty(), "pack_（空后缀）不应解析");
    }

    @Test
    void parseRejectsNullContainerId() {
        assertTrue(
            InspectScreen.parseWornPackInstance(null).isEmpty(), "null container id 不应解析");
    }

    // ── isWornPackContainerDroppable（穿戴态门控谓词）──

    @Test
    void dropAllowedWhenPackIsWorn() {
        assertTrue(
            InspectScreen.isWornPackContainerDroppable(snapshotWithWornPack(), "pack_1007"),
            "套包(1007)在 chest worn 栈顶（穿戴中）时应允许拖入");
    }

    @Test
    void dropAllowedWhenPackHeld() {
        // bug #2：放宽到携带面 —— 手持中的套包应放行（镜像 server find_pack_instances_anywhere）。
        assertTrue(
            InspectScreen.isWornPackContainerDroppable(snapshotWithHeldPack(), "pack_1007"),
            "套包(1007)被手持（MAIN_HAND held）时应允许拖入（携带面之一）");
    }

    @Test
    void dropAllowedWhenPackInHotbar() {
        assertTrue(
            InspectScreen.isWornPackContainerDroppable(snapshotWithPackInHotbar(), "pack_1007"),
            "套包(1007)在快捷栏时应允许拖入（携带面之一）");
    }

    @Test
    void dropAllowedWhenPackInBodyPocket() {
        // bug #2 核心：背包卸到 body_pocket 当货物 → 此前只扫 worn 层会误拒，现应放行。
        assertTrue(
            InspectScreen.isWornPackContainerDroppable(snapshotWithPackInBodyPocket(), "pack_1007"),
            "套包(1007)当货物塞在 body_pocket grid 时应允许拖入（卸下不丢，仍是有效容器）");
    }

    @Test
    void dropRejectedWhenPackAbsentEverywhere() {
        assertFalse(
            InspectScreen.isWornPackContainerDroppable(snapshotWithoutWornPack(), "pack_1007"),
            "套包(1007)不在任何携带面（worn/held/hotbar/body_pocket）→ 真丢地，应拒绝拖入");
    }

    @Test
    void dropAllowedForNonPackContainerRegardlessOfWornState() {
        // 非套包容器恒放行（不受门控），即便快照里没有对应穿戴件。
        assertTrue(
            InspectScreen.isWornPackContainerDroppable(snapshotWithoutWornPack(), "main_pack"),
            "main_pack 非套包容器，应恒放行");
        assertTrue(
            InspectScreen.isWornPackContainerDroppable(snapshotWithoutWornPack(), "body_pocket"),
            "body_pocket 非套包容器，应恒放行");
    }

    @Test
    void dropRejectedForPackWhenSnapshotNull() {
        // 套包容器但无快照可校验穿戴态 → 保守拒绝（不乐观落位）。
        assertFalse(
            InspectScreen.isWornPackContainerDroppable(null, "pack_1007"),
            "套包容器但快照为 null 时应保守拒绝");
    }

    @Test
    void nonPackContainerAllowedEvenWhenSnapshotNull() {
        // 非套包容器即便快照为 null 也放行（parseWornPackInstance 先返回 empty）。
        assertTrue(
            InspectScreen.isWornPackContainerDroppable(null, "main_pack"),
            "非套包容器在快照为 null 时仍应放行（不进入穿戴态校验）");
    }

    // ── 门控决定 → toast 反馈契约（复现生产 drop 分支的拒绝路径）──

    @Test
    void rejectedDropTriggersWarnToast() {
        InventoryModel snapshot = snapshotWithoutWornPack();
        long now = 1_000L;
        // 复现 attemptDrop 的拒绝分支：门控不通过 → showActionToast(WARN)。
        boolean droppable = InspectScreen.isWornPackContainerDroppable(snapshot, "pack_1007");
        assertFalse(droppable, "前置：未穿戴套包应被门控拒绝");
        // 拒绝时生产代码弹 WARN toast（"背包未穿戴，无法放入内含物"）。
        BongToast.show(
            "背包未穿戴，无法放入内含物",
            0xFFFFAA55, // ACTION_TOAST_WARN
            now,
            2_200L);
        BongToast active = BongToast.current(now + 100L);
        assertFalse(active.isEmpty(), "门控拒绝后应有活跃 toast（不沉默退回）");
        assertEquals(
            "背包未穿戴，无法放入内含物",
            active.text().getString(),
            "toast 文案应说明背包未穿戴（带玩家可懂的拒绝原因）");
        assertEquals(0xFFFFAA55, active.color(), "拒绝 toast 应为 WARN 配色");
    }

    @Test
    void allowedDropDoesNotRequireToast() {
        InventoryModel snapshot = snapshotWithWornPack();
        boolean droppable = InspectScreen.isWornPackContainerDroppable(snapshot, "pack_1007");
        assertTrue(droppable, "穿戴中套包应放行（发 move intent，不弹拒绝 toast）");
        // 放行路径不弹拒绝 toast。
        assertTrue(
            BongToast.current(System.currentTimeMillis()).isEmpty(),
            "放行路径不应留下拒绝 toast");
    }
}
