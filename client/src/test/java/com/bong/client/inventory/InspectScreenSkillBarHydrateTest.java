package com.bong.client.inventory;

import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.ClientRequestSender;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

/**
 * plan-block-placement-ux-v1 P1 — 放置消耗实例后，SkillBar 槽按 inventory_snapshot 自洽刷新。
 *
 * <p>背景（为何由客户端自洽，而非依赖服务端重推）：服务端 {@code skillbar_config} emit 用
 * {@code Changed<SkillBarBindings>} 触发，而方块放置消耗（{@code consume_item_instance_once}）只动
 * 库存、不改 {@code SkillBarBindings} 组件（其槽仍持悬空 instance_id），故服务端<b>不会</b>主动重推
 * 清空。客户端在每次 inventory_snapshot 落地后（{@code populateFromModel → hydrateSkillBarFromStore}）
 * 用 authoritative 库存校验每个 ITEM 槽绑定：
 * <ul>
 *   <li>实例已不存在（最后一个被消耗）→ 清槽 entry + selectedSlot 失效复位；</li>
 *   <li>实例仍在（count 递减但 >0）→ 保留绑定（count 显示由 inventory_snapshot 驱动）；</li>
 *   <li>SKILL 槽 / 空槽不受库存影响；</li>
 *   <li>model 尚未加载（empty）→ 不误清，待真 snapshot 校准。</li>
 * </ul>
 *
 * <p>用 {@link InspectScreen#reconcileSkillBarForTests}（设 model + 仅跑 hydrateSkillBarFromStore）
 * 驱动收口，断言 {@link SkillBarStore} 槽 / selectedSlot 这些<b>可观察客户端状态</b>。槽位视觉
 * （hotbarSlots 图标）由 owo build() 生命周期才实例化，headless 不可用，故视觉由 e2e 覆盖。
 */
class InspectScreenSkillBarHydrateTest {

    @AfterEach
    void tearDown() {
        SkillBarStore.resetForTests();
        ClientRequestSender.resetBackendForTests();
    }

    /** authoritative（非空）库存，仅含境界（无目标方块）——模拟「放置耗尽方块后」的快照。 */
    private static InventoryModel authoritativeWithoutBlock() {
        return InventoryModel.builder()
            .cultivation("引气", 10.0, 100.0, 1.0)
            .build();
    }

    // ── 单实例方块绑定后被放置消耗 → 清槽 ──

    @Test
    void consumedSoleInstanceClearsBoundSlot() {
        SkillBarStore.updateSlot(2, SkillBarEntry.item("earth_crumb", "土块", 0, 0, null));
        SkillBarStore.setSelectedSlot(2);

        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        screen.reconcileSkillBarForTests(authoritativeWithoutBlock());

        assertNull(SkillBarStore.snapshot().slot(2),
            "绑定方块被放置耗尽最后一个实例后，SkillBarStore.slot(2) 应被清为 null，"
                + "不得用占位名硬撑残留图标");
        assertEquals(SkillBarStore.NO_SELECTED_SLOT, SkillBarStore.selectedSlot(),
            "清槽后选中槽应随 clearInvalidSelectedSlot 复位为 NO_SELECTED_SLOT，避免热键指向空槽");
    }

    // ── 实例仍在（堆叠 count>1，放置后递减）→ 保留绑定、选中保持 ──

    @Test
    void survivingStackKeepsBindingAndSelection() {
        SkillBarStore.updateSlot(0, SkillBarEntry.item("earth_crumb", "土块", 0, 0, null));
        SkillBarStore.setSelectedSlot(0);

        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        // 放置前 stackCount=5，放置一次后服务端回写 stackCount=4 的同实例（实例仍在）。
        InventoryItem remaining = InventoryItem.createFull(
            1L, "earth_crumb", "土块", 1, 1, 0.1, "common", "可放置的土块", 4, 1.0, 1.0);
        screen.reconcileSkillBarForTests(InventoryModel.builder()
            .cultivation("引气", 10.0, 100.0, 1.0)
            .gridItem(remaining, 0, 0)
            .build());

        SkillBarEntry entry = SkillBarStore.snapshot().slot(0);
        assertNotNull(entry, "实例仍在库存（count>0）时绑定应保留，不得误清");
        assertEquals("earth_crumb", entry.id());
        assertEquals(SkillBarEntry.Kind.ITEM, entry.kind());
        assertEquals(0, SkillBarStore.selectedSlot(),
            "实例仍在时选中槽应保持，不复位（玩家可继续按该槽放置）");
    }

    // ── count 扣减：count=1 仍在保留，下一次 snapshot 删除后清槽（状态转换 1→0）──

    @Test
    void stackDepletedToZeroClearsSlotOnNextSnapshot() {
        SkillBarStore.updateSlot(3, SkillBarEntry.item("earth_crumb", "土块", 0, 0, null));

        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        // 第一次：count=1 仍在 → 保留。
        InventoryItem lastOne = InventoryItem.createFull(
            1L, "earth_crumb", "土块", 1, 1, 0.1, "common", "", 1, 1.0, 1.0);
        screen.reconcileSkillBarForTests(InventoryModel.builder()
            .cultivation("引气", 10.0, 100.0, 1.0)
            .gridItem(lastOne, 0, 0)
            .build());
        assertNotNull(SkillBarStore.snapshot().slot(3),
            "count=1 时实例仍在，绑定应保留");

        // 第二次：放置后实例被删，库存仅余境界 → 清槽。
        screen.reconcileSkillBarForTests(authoritativeWithoutBlock());
        assertNull(SkillBarStore.snapshot().slot(3),
            "最后一个被消耗、实例删除后，下一次 snapshot 应清槽");
    }

    // ── 边界：model 尚未加载（empty）时不误清绑定 ──

    @Test
    void emptyModelDoesNotClearBindingPrematurely() {
        SkillBarStore.updateSlot(1, SkillBarEntry.item("earth_crumb", "土块", 0, 0, null));

        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        // 显式以 empty model reconcile（模拟首帧 / 首个 snapshot 未到达）。
        screen.reconcileSkillBarForTests(InventoryModel.empty());

        assertNotNull(SkillBarStore.snapshot().slot(1),
            "model 为空（未加载 authoritative snapshot）时不得误清绑定，"
                + "防开屏首帧把服务端刚下发的有效绑定抹掉");
        assertEquals("earth_crumb", SkillBarStore.snapshot().slot(1).id());
    }

    @Test
    void emptyModelThenAuthoritativeSnapshotClears() {
        SkillBarStore.updateSlot(1, SkillBarEntry.item("earth_crumb", "土块", 0, 0, null));
        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        screen.reconcileSkillBarForTests(InventoryModel.empty());
        assertNotNull(SkillBarStore.snapshot().slot(1), "首帧空 model 不清");

        // authoritative snapshot 落地且物品确实不在 → 这次才清。
        screen.reconcileSkillBarForTests(authoritativeWithoutBlock());
        assertNull(SkillBarStore.snapshot().slot(1),
            "真 authoritative snapshot 落地后，确认物品不在库存 → 清槽");
    }

    // ── SKILL 槽不受库存影响 ──

    @Test
    void skillSlotSurvivesInventoryReconcile() {
        SkillBarStore.updateSlot(5, SkillBarEntry.skill("fireball", "火球术", 1000, 2000, "icon.png"));

        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        screen.reconcileSkillBarForTests(authoritativeWithoutBlock());

        SkillBarEntry entry = SkillBarStore.snapshot().slot(5);
        assertNotNull(entry, "SKILL 槽与库存物品无关，不应被 inventory 校验清掉");
        assertEquals(SkillBarEntry.Kind.SKILL, entry.kind());
        assertEquals("fireball", entry.id());
    }

    // ── 空槽保持空，不被错误填充 ──

    @Test
    void emptySlotStaysEmptyAfterReconcile() {
        InspectScreen screen = new InspectScreen(InventoryModel.empty());
        screen.reconcileSkillBarForTests(authoritativeWithoutBlock());

        for (int i = 0; i < 9; i++) {
            assertNull(SkillBarStore.snapshot().slot(i),
                "无绑定时槽 " + i + " 应保持 null，不被 reconcile 误填");
        }
    }

    // ── 多槽混合：一槽消耗清空、一槽仍在保留、一槽 SKILL 不动 ──

    @Test
    void mixedSlotsReconcileIndependently() {
        SkillBarStore.updateSlot(0, SkillBarEntry.item("earth_crumb", "土块", 0, 0, null));   // 将被消耗
        SkillBarStore.updateSlot(1, SkillBarEntry.item("barren_sand", "沙", 0, 0, null));    // 仍在
        SkillBarStore.updateSlot(2, SkillBarEntry.skill("fireball", "火球", 0, 0, null));     // SKILL

        InspectScreen screen = new InspectScreen(InventoryModel.empty());

        InventoryItem sand = InventoryItem.createFull(
            2L, "barren_sand", "沙", 1, 1, 0.1, "common", "", 3, 1.0, 1.0);
        screen.reconcileSkillBarForTests(InventoryModel.builder()
            .cultivation("引气", 10.0, 100.0, 1.0)
            .gridItem(sand, 0, 0)
            .build());

        assertNull(SkillBarStore.snapshot().slot(0),
            "earth_crumb 不在库存 → 槽 0 清空");
        assertNotNull(SkillBarStore.snapshot().slot(1),
            "barren_sand 仍在库存 → 槽 1 保留");
        assertEquals("barren_sand", SkillBarStore.snapshot().slot(1).id());
        assertNotNull(SkillBarStore.snapshot().slot(2),
            "SKILL 槽 2 不受库存影响 → 保留");
        assertEquals(SkillBarEntry.Kind.SKILL, SkillBarStore.snapshot().slot(2).kind());
    }
}
