package com.bong.client.inventory;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.InventoryModel.ContainerDef;
import com.bong.client.inventory.model.SlotContents;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 容器 tab 刷新判定 + A 准则（worn-tab）回归。
 *
 * <p><b>A 准则</b>（收窄 #778「pack 一律不显 tab」）：{@code pack_<数字>} 背包件容器
 * <b>仅当 owner 背包件当前穿戴在某身体槽 worn 层</b>才进顶层 tab；非 worn（当货物躺 body_pocket /
 * 手持 held / 已落地）→ 不进 tab，仅经双击/右键悬浮窗访问。tab 与悬浮窗入口并存。本测试锁住
 * {@code computeContainerDefs}（worn→含 / 非worn/owner==null→不含 / body_pocket 恒 index 0）、
 * {@code isPackOwnerInWornSlot}（worn 命中 / held 不命中 / 叠穿非栈顶命中 / 空槽边界）、
 * {@code containerDefsDiffer}（穿/卸 pack 改变 tab 成员 → 结构变化触发 rebuild）。
 * {@code rebuildContainerSection} 的 owo 布局重建 headless 无法验，由 gradle build + 真机兜底。</p>
 */
public class InspectScreenContainerRefreshTest {

    /** 无 owner 的 pack def（owner==null，模拟旧 server 缺字段 / 当货物）。 */
    private static ContainerDef pack(String id, int rows, int cols) {
        return new ContainerDef(id, "背包", rows, cols);
    }

    /** 带 owner 的 pack def（owner=instanceId，镜像 server 下发 owner_instance_id）。 */
    private static ContainerDef packOwned(long ownerId, int rows, int cols) {
        return new ContainerDef("pack_" + ownerId, "破草包", rows, cols, ownerId, false);
    }

    /** 非 pack 的静态容器（如 main_pack）——恒作为 tab。 */
    private static ContainerDef staticContainer(String id, int rows, int cols) {
        return new ContainerDef(id, "静态容器", rows, cols);
    }

    private static ContainerDef bodyPocket() {
        return new ContainerDef(InventoryModel.BODY_POCKET_CONTAINER_ID, "贴身口袋", 2, 3);
    }

    private static InventoryItem packItem(long instanceId) {
        return InventoryItem.createFull(
            instanceId, "worn_grass_pouch", "破草包", 2, 2, 0.3, "common", "", 1, 1.0, 1.0);
    }

    private static InventoryModel modelWith(List<ContainerDef> defs) {
        return InventoryModel.builder().containers(defs).build();
    }

    /** 把 owner=ownerId 的背包件**穿戴**在 chest 槽 worn 层，并把其 pack 容器 def 一并给出。 */
    private static InventoryModel modelWithWornPack(long ownerId, ContainerDef... extra) {
        List<ContainerDef> defs = new ArrayList<>();
        defs.add(bodyPocket());
        defs.add(packOwned(ownerId, 3, 3));
        for (ContainerDef d : extra) defs.add(d);
        return InventoryModel.builder()
            .containers(defs)
            .equip(EquipSlotType.CHEST, packItem(ownerId))
            .build();
    }

    // ── A 准则：worn pack → 进 tab；非 worn / owner==null → 不进 ──

    @Test
    void wornPackIsIncludedInTabDefs() {
        // owner=1007 的背包件穿在 chest worn 层 → pack_1007 进 tab（A 准则：穿戴态显常驻 tab）。
        var defs = InspectScreen.computeContainerDefs(modelWithWornPack(1007L));
        assertTrue(defs.stream().anyMatch(d -> "pack_1007".equals(d.id())),
            "owner 穿戴在 worn 层时，pack_1007 应进 tab（A 准则收窄 #778）");
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, defs.get(0).id(),
            "body_pocket 恒 index 0（决议 #18 不变）");
    }

    @Test
    void nonWornPackIsExcludedFromTabDefs() {
        // 同样 pack_2007 容器存在且带 owner=2007，但 owner 不在任何 worn 层（无 equip）→ 不进 tab。
        var defs = InspectScreen.computeContainerDefs(modelWith(List.of(
            bodyPocket(), packOwned(2007L, 3, 3))));
        assertFalse(defs.stream().anyMatch(d -> "pack_2007".equals(d.id())),
            "owner 不在 worn 层（当货物/手持/落地）→ pack 不进 tab，仅走悬浮窗");
        assertEquals(1, defs.size(), "仅剩 body_pocket");
    }

    @Test
    void ownerNullPackIsExcludedNotDegradedToShown() {
        // owner==null（旧 server 缺字段）→ skip，不退化为显示（保持 #778 不退化）。
        var defs = InspectScreen.computeContainerDefs(modelWith(List.of(
            bodyPocket(), pack("pack_1007", 4, 4))));
        assertFalse(defs.stream().anyMatch(d -> "pack_1007".equals(d.id())),
            "owner==null 的 pack 应 skip（旧 server 缺字段不退化为显示）");
    }

    @Test
    void heldPackOwnerDoesNotShowTab() {
        // owner 背包件在 main_hand 的 held（手持代表件），不算「穿戴上身」→ pack 不进 tab。
        InventoryModel m = InventoryModel.builder()
            .containers(List.of(bodyPocket(), packOwned(3007L, 3, 3)))
            .equipSlot(EquipSlotType.MAIN_HAND, SlotContents.ofHeld(packItem(3007L)))
            .build();
        var defs = InspectScreen.computeContainerDefs(m);
        assertFalse(defs.stream().anyMatch(d -> "pack_3007".equals(d.id())),
            "owner 在 held（手持）而非 worn → 不算穿戴上身，pack 不进 tab");
    }

    @Test
    void multiLayerWornNonTopPackStillShowsTab() {
        // 叠穿：worn 栈 [pack(底), 护甲(顶)]，owner 在非栈顶 → 仍应进 tab（验证用 worn() 全栈非 wornTop）。
        InventoryItem pack = packItem(4007L);
        InventoryItem armorTop = InventoryItem.createFull(
            9999L, "armor_iron_chest", "铁胸甲", 2, 2, 6.0, "common", "", 1, 1.0, 1.0);
        InventoryModel m = InventoryModel.builder()
            .containers(List.of(bodyPocket(), packOwned(4007L, 3, 3)))
            .equipSlot(EquipSlotType.CHEST, new SlotContents(List.of(pack, armorTop), null))
            .build();
        var defs = InspectScreen.computeContainerDefs(m);
        assertTrue(defs.stream().anyMatch(d -> "pack_4007".equals(d.id())),
            "owner 在 worn 栈中部（非栈顶）也应进 tab；漏判说明误用了 wornTop 而非 worn() 全栈");
    }

    @Test
    void bodyPocketStaysIndexZeroAlongsideWornPack() {
        var defs = InspectScreen.computeContainerDefs(
            modelWithWornPack(1007L, staticContainer("main_pack", 5, 7)));
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, defs.get(0).id(),
            "即便存在 worn pack + 静态容器，body_pocket 仍恒 index 0（决议 #18 回归）");
    }

    @Test
    void computeContainerDefsSynthesizesBodyPocketWhenServerOmits() {
        // server 只下发一个 owner==null pack（被排除）→ 仍补默认 body_pocket 占 index 0（防右侧面板塌）。
        var defs = InspectScreen.computeContainerDefs(modelWith(List.of(pack("pack_1007", 4, 4))));
        assertEquals(1, defs.size(), "pack 被排除后只剩合成的 body_pocket");
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, defs.get(0).id());
        assertEquals(2, defs.get(0).rows(), "合成 body_pocket 默认 2 行");
        assertEquals(3, defs.get(0).cols(), "合成 body_pocket 默认 3 列");
    }

    // ── isPackOwnerInWornSlot 直接边界 ──

    @Test
    void isPackOwnerInWornSlotMatchesWornMissesHeldAndEmpty() {
        InventoryModel worn = InventoryModel.builder()
            .equip(EquipSlotType.CHEST, packItem(7001L))
            .build();
        assertTrue(InspectScreen.isPackOwnerInWornSlot(worn, 7001L),
            "owner 在 worn 层应命中");
        assertFalse(InspectScreen.isPackOwnerInWornSlot(worn, 9998L),
            "不存在的 owner 不命中");

        InventoryModel held = InventoryModel.builder()
            .equipSlot(EquipSlotType.MAIN_HAND, SlotContents.ofHeld(packItem(7002L)))
            .build();
        assertFalse(InspectScreen.isPackOwnerInWornSlot(held, 7002L),
            "owner 仅在 held（手持）不算穿戴，不命中");

        assertFalse(InspectScreen.isPackOwnerInWornSlot(InventoryModel.empty(), 7001L),
            "空装备态（无任何 worn）不命中");
    }

    // ── containerDefsDiffer：穿/卸 pack 改变 tab 成员 = 结构变化 ──

    @Test
    void equippingPackIsStructuralChange() {
        // A 准则后：卸下态（pack 不在 tab）→ 穿戴态（pack_1007 进 tab）= tab 成员变化 → 结构变化（需重建）。
        var unworn = InspectScreen.computeContainerDefs(modelWith(List.of(
            bodyPocket(), packOwned(1007L, 3, 3))));
        var worn = InspectScreen.computeContainerDefs(modelWithWornPack(1007L));
        assertTrue(InspectScreen.containerDefsDiffer(worn, unworn),
            "穿上 pack 使其进 tab（成员集合变化）→ 必判结构变化以重建 tab 栏");
    }

    @Test
    void addingStaticContainerIsStructuralChange() {
        var before = InspectScreen.computeContainerDefs(modelWith(List.of(bodyPocket())));
        var after = InspectScreen.computeContainerDefs(modelWith(List.of(
            bodyPocket(), staticContainer("main_pack", 5, 7))));
        assertTrue(InspectScreen.containerDefsDiffer(after, before),
            "新增非 pack 静态容器（main_pack）必判结构变化（需重建 tab）");
    }

    @Test
    void staticContainerResizeIsStructuralChange() {
        assertTrue(
            InspectScreen.containerDefsDiffer(
                List.of(staticContainer("main_pack", 2, 2)),
                List.of(staticContainer("main_pack", 5, 7))),
            "同 id 静态容器换尺寸（2×2→5×7）必判结构变化，否则 grid 尺寸错位");
    }

    @Test
    void contentsOnlyChangeIsNotStructural() {
        var a = InspectScreen.computeContainerDefs(modelWith(List.of(
            bodyPocket(), staticContainer("main_pack", 5, 7))));
        var b = InspectScreen.computeContainerDefs(modelWith(List.of(
            bodyPocket(), staticContainer("main_pack", 5, 7))));
        assertFalse(InspectScreen.containerDefsDiffer(a, b),
            "容器集合不变（仅内含物可能变）不应判为结构变化，避免每次 snapshot 都重建 tab 抖动");
    }
}
