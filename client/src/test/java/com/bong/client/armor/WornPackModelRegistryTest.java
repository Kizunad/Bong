package com.bong.client.armor;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** plan-tarkov-backpack-v1 P4 — WornPackModelRegistry happy + 缺失分支 pin。 */
class WornPackModelRegistryTest {

    @Test
    void wornGrassPouchMapsToBackModelWithGrassPouchTexture() {
        Optional<WornPackModelRegistry.WornPackModelSpec> spec =
            WornPackModelRegistry.get("worn_grass_pouch");
        assertTrue(spec.isPresent(), "worn_grass_pouch 应注册可上身渲染规格，实际缺失");
        assertEquals("worn_grass_pouch", spec.get().templateId());
        assertEquals(WornPackModelRegistry.WornPackModelKind.BACK, spec.get().kind(),
            "破草包默认挂背面 BACK（front 留 P5）");
        assertEquals(new Identifier("bong", "textures/entity/grass_pouch.png"),
            spec.get().textureId(),
            "贴图必须是既有 entity 贴图 grass_pouch.png（非 GUI 图标）");
    }

    @Test
    void grassPouchAliasAlsoRegistered() {
        Optional<WornPackModelRegistry.WornPackModelSpec> spec =
            WornPackModelRegistry.get("grass_pouch");
        assertTrue(spec.isPresent(), "grass_pouch 别名应同样注册（镜像 InventoryEquipRules 白名单）");
        assertEquals(WornPackModelRegistry.WornPackModelKind.BACK, spec.get().kind());
    }

    @Test
    void nonContainerTemplateReturnsEmpty() {
        assertTrue(WornPackModelRegistry.get("iron_chestplate").isEmpty(),
            "护甲模板不在背包件表，应 empty（否则会把护甲当背包渲染）");
        assertTrue(WornPackModelRegistry.get("disguise_wrap").isEmpty(),
            "伪皮模板不在背包件表，应 empty");
    }

    @Test
    void nullAndBlankTemplateReturnEmpty() {
        assertTrue(WornPackModelRegistry.get(null).isEmpty(), "null 模板应 empty 不抛");
        assertTrue(WornPackModelRegistry.get("").isEmpty(), "空串模板应 empty");
        assertTrue(WornPackModelRegistry.get("   ").isEmpty(), "纯空白模板应 empty");
    }

    @Test
    void sizeCoversBothKnownContainerTemplates() {
        assertEquals(2, WornPackModelRegistry.size(),
            "当前仓库仅 worn_grass_pouch / grass_pouch 两个 container 模板，期望 2");
        assertFalse(WornPackModelRegistry.get("worn_grass_pouch").isEmpty());
    }
}
