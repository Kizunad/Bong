package com.bong.client.armor;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.SlotContents;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-tarkov-backpack-v1 P4 — WornPackFeatureRenderer 过滤谓词 pin。
 *
 * <p>核心契约：CHEST 穿戴层含护甲 + 伪皮 + 背包件时，**仅**背包件（container_spec != null，
 * 经 InventoryEquipRules.isContainerPublic 判定）被本 renderer 拾取；护甲 / 伪皮**不**被拾取
 *（否则会把护甲 / 伪皮当背包渲染出破草包模型）。
 */
class WornPackFeatureRendererTest {

    private static InventoryItem item(String id) {
        return InventoryItem.simple(id, id);
    }

    @Test
    void chestWithArmorFalseSkinAndPackRendersOnlyTheContainer() {
        // 栈底→栈顶：护甲、伪皮、背包件混在 CHEST worn 栈。
        SlotContents chest = new SlotContents(
            List.of(item("iron_chestplate"), item("disguise_wrap"), item("worn_grass_pouch")),
            null
        );
        List<WornPackModelRegistry.WornPackModelSpec> specs =
            WornPackFeatureRenderer.collectRenderable(chest);

        assertEquals(1, specs.size(),
            "护甲 + 伪皮 + 背包件混栈，应只渲染 1 个背包件，实际数量=" + specs.size());
        assertEquals("worn_grass_pouch", specs.get(0).templateId(),
            "被拾取的必须是背包件 worn_grass_pouch，而非护甲 / 伪皮");
        assertTrue(specs.stream().noneMatch(s -> s.templateId().equals("iron_chestplate")),
            "护甲(container_spec==null)绝不能被上身背包 renderer 拾取");
        assertTrue(specs.stream().noneMatch(s -> s.templateId().equals("disguise_wrap")),
            "伪皮(container_spec==null)绝不能被上身背包 renderer 拾取");
    }

    @Test
    void chestWithOnlyArmorRendersNothing() {
        SlotContents chest = SlotContents.ofWorn(item("iron_chestplate"));
        assertTrue(WornPackFeatureRenderer.collectRenderable(chest).isEmpty(),
            "只穿护甲时本 renderer 应不渲染任何件（不画护甲）");
    }

    @Test
    void emptyChestRendersNothing() {
        assertTrue(WornPackFeatureRenderer.collectRenderable(SlotContents.empty()).isEmpty());
    }

    @Test
    void nullChestRendersNothing() {
        assertTrue(WornPackFeatureRenderer.collectRenderable(null).isEmpty(),
            "CHEST 槽缺省(null)应安全返回空表，不抛");
    }

    @Test
    void grassPouchAliasIsAlsoRendered() {
        SlotContents chest = SlotContents.ofWorn(item("grass_pouch"));
        List<WornPackModelRegistry.WornPackModelSpec> specs =
            WornPackFeatureRenderer.collectRenderable(chest);
        assertEquals(1, specs.size());
        assertEquals("grass_pouch", specs.get(0).templateId());
    }

    @Test
    void multipleWornContainersAllRendered() {
        // 边界：穿戴层叠两件背包件（worn cap=3 允许），两件都应渲染。
        SlotContents chest = new SlotContents(
            List.of(item("worn_grass_pouch"), item("grass_pouch")),
            null
        );
        List<WornPackModelRegistry.WornPackModelSpec> specs =
            WornPackFeatureRenderer.collectRenderable(chest);
        assertEquals(2, specs.size(), "两件穿戴背包件应各渲染一次");
    }
}
