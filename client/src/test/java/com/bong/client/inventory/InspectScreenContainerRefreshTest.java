package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.InventoryModel.ContainerDef;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-tarkov-backpack-v1 后续修复 —— InspectScreen 容器 tab 栏随 snapshot 实时刷新的判定逻辑回归。
 *
 * <p>真机症状：卸下背包后右侧容器 tab 没自己刷新，重开界面才正确。根因：snapshot listener
 * 只 {@code populateFromModel} 重填已有格子，不重建 tab 栏；缺 structuralChange 判定。本测试锁住
 * {@code computeContainerDefs}（body_pocket 恒置 index 0）+ {@code containerDefsDiffer}（容器增/删/
 * 换尺寸 = 结构变化，仅内含物变化 ≠ 结构变化）的核心决策。{@code rebuildContainerSection} 的 owo
 * 布局重建 headless 无法验，由 gradle build + 真机验证兜底。</p>
 */
public class InspectScreenContainerRefreshTest {

    private static ContainerDef pack(String id, int rows, int cols) {
        return new ContainerDef(id, "背包", rows, cols);
    }

    private static ContainerDef bodyPocket() {
        return new ContainerDef(InventoryModel.BODY_POCKET_CONTAINER_ID, "贴身口袋", 2, 3);
    }

    private static InventoryModel modelWith(List<ContainerDef> defs) {
        return InventoryModel.builder().containers(defs).build();
    }

    @Test
    void computeContainerDefsAlwaysPutsBodyPocketFirst() {
        // server 下发顺序 [pack, body_pocket] → 计算后 body_pocket 必在 tab index 0（决议 #18）。
        var defs = InspectScreen.computeContainerDefs(modelWith(List.of(
            pack("pack_1007", 4, 4), bodyPocket()
        )));
        assertEquals(2, defs.size(), "应保留 2 个容器（body_pocket + pack）");
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, defs.get(0).id(),
            "body_pocket 必须置 tab index 0（决议 #18），不受 server 下发顺序影响");
        assertEquals("pack_1007", defs.get(1).id(), "pack 排 body_pocket 之后");
    }

    @Test
    void computeContainerDefsSynthesizesBodyPocketWhenServerOmits() {
        // server 未下发 body_pocket → 补默认 2×3 占 index 0（防右侧面板塌）。
        var defs = InspectScreen.computeContainerDefs(modelWith(List.of(pack("pack_1007", 4, 4))));
        assertEquals(2, defs.size(), "缺失的 body_pocket 应被补上");
        assertEquals(InventoryModel.BODY_POCKET_CONTAINER_ID, defs.get(0).id());
        assertEquals(2, defs.get(0).rows(), "合成 body_pocket 默认 2 行");
        assertEquals(3, defs.get(0).cols(), "合成 body_pocket 默认 3 列");
    }

    @Test
    void unequippingPackIsStructuralChange() {
        // 穿包态 [body_pocket, pack] → 卸包后 [body_pocket]：容器数变 → 必须重建 tab（修真机 bug）。
        var withPack = InspectScreen.computeContainerDefs(modelWith(List.of(bodyPocket(), pack("pack_1007", 4, 4))));
        var withoutPack = InspectScreen.computeContainerDefs(modelWith(List.of(bodyPocket())));
        assertTrue(InspectScreen.containerDefsDiffer(withoutPack, withPack),
            "卸下背包后容器集合 [body_pocket] 与穿包态 [body_pocket, pack] 必判为结构变化，否则 pack tab 残留");
    }

    @Test
    void equippingPackIsStructuralChange() {
        // 卸包态 [body_pocket] → 穿包后 [body_pocket, pack]：多出容器 → 必须重建 tab。
        var withoutPack = InspectScreen.computeContainerDefs(modelWith(List.of(bodyPocket())));
        var withPack = InspectScreen.computeContainerDefs(modelWith(List.of(bodyPocket(), pack("pack_1007", 4, 4))));
        assertTrue(InspectScreen.containerDefsDiffer(withPack, withoutPack),
            "穿上背包后多出 pack 容器必判为结构变化（否则双击无 tab 可开）");
    }

    @Test
    void packResizeIsStructuralChange() {
        // 同 id 但 rows/cols 变（升级换品）→ 结构变化（grid 尺寸要重建）。
        assertTrue(
            InspectScreen.containerDefsDiffer(List.of(pack("pack_1007", 2, 2)), List.of(pack("pack_1007", 4, 4))),
            "同 id 背包换尺寸（2×2→4×4）必判为结构变化，否则 grid 尺寸错位");
    }

    @Test
    void contentsOnlyChangeIsNotStructural() {
        // 容器集合完全相同（id+rows+cols 一致）→ 非结构变化（内含物变动走 populateFromModel，免无谓重建）。
        var a = InspectScreen.computeContainerDefs(modelWith(List.of(bodyPocket(), pack("pack_1007", 4, 4))));
        var b = InspectScreen.computeContainerDefs(modelWith(List.of(bodyPocket(), pack("pack_1007", 4, 4))));
        assertFalse(InspectScreen.containerDefsDiffer(a, b),
            "容器集合不变（仅内含物可能变）不应判为结构变化，避免每次 snapshot 都重建 tab 抖动");
    }
}
