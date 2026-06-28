package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.InventoryModel.ContainerDef;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-tarkov-floating-windows bug #1 + 容器 tab 刷新判定回归。
 *
 * <p><b>bug #1</b>：背包件容器 {@code pack_<数字>} 不再占顶层 tab —— 无论 worn 还是当货物塞 body_pocket，
 * 统一经双击/右键开悬浮窗访问。tab 栏只代表固定挂载的基础容器（body_pocket 等）。本测试锁住
 * {@code computeContainerDefs} 过滤 pack_ + body_pocket 恒 index 0；并锁住 {@code containerDefsDiffer}
 * 对<b>非 pack 静态容器</b>的增/删/换尺寸仍判结构变化（pack 增删因不进 tab 已不触发 rebuild，pack
 * 悬浮窗的关闭改由 listener 每帧 reconcile，见 InspectScreen 注释）。{@code rebuildContainerSection}
 * 的 owo 布局重建 headless 无法验，由 gradle build + 真机验证兜底。</p>
 */
public class InspectScreenContainerRefreshTest {

    private static ContainerDef pack(String id, int rows, int cols) {
        return new ContainerDef(id, "背包", rows, cols);
    }

    /** 非 pack 的静态容器（如 main_pack）——仍作为 tab。 */
    private static ContainerDef staticContainer(String id, int rows, int cols) {
        return new ContainerDef(id, "静态容器", rows, cols);
    }

    private static ContainerDef bodyPocket() {
        return new ContainerDef(InventoryModel.BODY_POCKET_CONTAINER_ID, "贴身口袋", 2, 3);
    }

    private static InventoryModel modelWith(List<ContainerDef> defs) {
        return InventoryModel.builder().containers(defs).build();
    }

    // ── bug #1：pack_<数字> 不进 tab defs ──

    @Test
    void computeContainerDefsExcludesWornPackContainers() {
        // server 下发 [pack_1007, body_pocket] → tab defs 仅 body_pocket（pack 不建顶层 tab）。
        var defs = InspectScreen.computeContainerDefs(modelWith(List.of(
            pack("pack_1007", 4, 4), bodyPocket()
        )));
        assertEquals(1, defs.size(),
            "pack_<数字> 容器不进 tab，仅剩 body_pocket（bug #1：背包统一走悬浮窗）");
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, defs.get(0).id(),
            "唯一 tab 应是 body_pocket（恒 index 0）");
    }

    @Test
    void computeContainerDefsExcludesAllPacksKeepsStaticContainers() {
        // 多个 pack + body_pocket + 一个非 pack 静态容器 → defs 不含任一 pack_，静态容器保留。
        var defs = InspectScreen.computeContainerDefs(modelWith(List.of(
            pack("pack_1", 3, 3),
            staticContainer("main_pack", 5, 7),
            pack("pack_2", 2, 2),
            bodyPocket()
        )));
        assertEquals(2, defs.size(), "应只剩 body_pocket + main_pack（两个 pack_ 全被排除）");
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, defs.get(0).id(),
            "body_pocket 永远排首位（index 0）");
        boolean hasAnyPack = defs.stream()
            .anyMatch(d -> InspectScreen.parseWornPackInstance(d.id()).isPresent());
        assertFalse(hasAnyPack, "tab defs 不应含任何 pack_<数字> 容器");
        assertTrue(defs.stream().anyMatch(d -> "main_pack".equals(d.id())),
            "非 pack 静态容器 main_pack 应保留为 tab");
    }

    @Test
    void computeContainerDefsSynthesizesBodyPocketWhenServerOmits() {
        // server 只下发一个 pack（被排除）→ 仍补默认 body_pocket 占 index 0（防右侧面板塌）。
        var defs = InspectScreen.computeContainerDefs(modelWith(List.of(pack("pack_1007", 4, 4))));
        assertEquals(1, defs.size(), "pack 被排除后只剩合成的 body_pocket");
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, defs.get(0).id());
        assertEquals(2, defs.get(0).rows(), "合成 body_pocket 默认 2 行");
        assertEquals(3, defs.get(0).cols(), "合成 body_pocket 默认 3 列");
    }

    // ── containerDefsDiffer：非 pack 静态容器增/删/换尺寸 = 结构变化 ──

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
    void equippingOrUnequippingPackIsNotStructuralChange() {
        // pack 不进 tab：穿/卸 pack 时 tab defs 不变（都只剩 body_pocket）→ 非结构变化（不重建 tab）。
        // pack 悬浮窗的关闭由 listener 每帧 reconcile 负责，不依赖 structuralChange。
        var withPack = InspectScreen.computeContainerDefs(modelWith(List.of(
            bodyPocket(), pack("pack_1007", 4, 4))));
        var withoutPack = InspectScreen.computeContainerDefs(modelWith(List.of(bodyPocket())));
        assertFalse(InspectScreen.containerDefsDiffer(withPack, withoutPack),
            "穿/卸 pack 不改变 tab defs（pack 不进 tab）→ 不应判为结构变化，避免无谓重建 tab 抖动");
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
