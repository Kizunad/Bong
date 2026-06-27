package com.bong.client.armor;

import net.minecraft.client.model.ModelPart;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-tarkov-backpack-v1 P4 — WornPackModel 与 grass_pouch_back.geo.json 一致性 pin +
 * Bedrock→vanilla y 转换数学 + 纯 vanilla bake 可行性（headless）。
 */
class WornPackModelTest {

    @Test
    void backCubesMatchGrassPouchBackGeoExactly() {
        List<WornPackModel.GeoCube> cubes = WornPackModel.GRASS_POUCH_BACK_CUBES;
        assertEquals(4, cubes.size(),
            "grass_pouch_back.geo.json 单 bone 含 4 cube，转写表必须等量（改 geo 同步本表）");

        // cube 0 — 主袋身：origin [-3,11.5,1.5] size [6,5.5,3] uv [0,0]
        assertCube(cubes.get(0), -3.0f, 11.5f, 1.5f, 6.0f, 5.5f, 3.0f, 0, 0);
        // cube 1 — 袋口翻盖：origin [-3.2,16.5,1] size [6.4,1,4] uv [0,10]
        assertCube(cubes.get(1), -3.2f, 16.5f, 1.0f, 6.4f, 1.0f, 4.0f, 0, 10);
        // cube 2 — 背带横条：origin [-3.2,14,4.5] size [6.4,2.5,0.5] uv [0,16]
        assertCube(cubes.get(2), -3.2f, 14.0f, 4.5f, 6.4f, 2.5f, 0.5f, 0, 16);
        // cube 3 — 系绳扣：origin [-0.5,14.5,4.8] size [1,1,0.4] uv [18,0]
        assertCube(cubes.get(3), -0.5f, 14.5f, 4.8f, 1.0f, 1.0f, 0.4f, 18, 0);
    }

    @Test
    void bedrockToVanillaYFlipsAboutBodyTop() {
        // vanilla_y_min = 24 - (origin_y + size_y)：Bedrock 顶角(肩)→ vanilla 顶(y 最小)。
        assertEquals(7.0f, WornPackModel.bedrockToVanillaCuboidY(11.5f, 5.5f), 1e-4f,
            "主袋身 bedrock y∈[11.5,17] → vanilla 顶角 24-17=7");
        assertEquals(6.5f, WornPackModel.bedrockToVanillaCuboidY(16.5f, 1.0f), 1e-4f,
            "袋口 bedrock y_max=17.5 → vanilla 24-17.5=6.5");
        assertEquals(7.5f, WornPackModel.bedrockToVanillaCuboidY(14.0f, 2.5f), 1e-4f);
        assertEquals(8.5f, WornPackModel.bedrockToVanillaCuboidY(14.5f, 1.0f), 1e-4f);
    }

    @Test
    void shoulderHeightMapsToBodyTop() {
        // 肩(bedrock 24) → vanilla 0（body 顶）；腰(bedrock 12) → vanilla 12（body 底）。
        assertEquals(0.0f, WornPackModel.bedrockToVanillaCuboidY(24.0f, 0.0f), 1e-4f);
        assertEquals(12.0f, WornPackModel.bedrockToVanillaCuboidY(12.0f, 0.0f), 1e-4f);
    }

    @Test
    void textureDimsMatchGeo() {
        assertEquals(32, WornPackModel.TEXTURE_WIDTH);
        assertEquals(32, WornPackModel.TEXTURE_HEIGHT);
    }

    @Test
    void buildBackModelPartBakesSingleBoneVanillaModelPart() {
        // 纯 vanilla 数据构造，无 GeckoLib runtime / GL / 资源重载依赖 —— headless 可烘焙。
        ModelPart part = WornPackModel.buildBackModelPart();
        assertNotNull(part, "bake 必须返回非 null ModelPart（路线 (b) 的可行性 pin）");
        assertTrue(part.hasChild("grass_pouch_back"),
            "烘焙的 ModelPart 应含单 bone 'grass_pouch_back'（对应 geo 唯一 bone）");
        assertNotNull(part.getChild("grass_pouch_back"));
    }

    private static void assertCube(
        WornPackModel.GeoCube c,
        float ox, float oy, float oz, float sx, float sy, float sz, int u, int v
    ) {
        assertEquals(ox, c.ox(), 1e-4f, "origin x");
        assertEquals(oy, c.oy(), 1e-4f, "origin y");
        assertEquals(oz, c.oz(), 1e-4f, "origin z");
        assertEquals(sx, c.sx(), 1e-4f, "size x");
        assertEquals(sy, c.sy(), 1e-4f, "size y");
        assertEquals(sz, c.sz(), 1e-4f, "size z");
        assertEquals(u, c.u(), "uv u");
        assertEquals(v, c.v(), "uv v");
    }
}
