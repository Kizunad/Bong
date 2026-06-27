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

    @Test
    void convertedBoundingBoxSitsOnLowerBackWithinTorso() {
        // Round 2 结构化核验：把 4 cube 经 y 翻转后求 vanilla body 局部系包围盒，核验挂点/比例/不穿模。
        // body cuboid = addCuboid(-4,0,-2,8,12,4)：x∈[-4,4]、y∈[0,12](颈→腰)、z∈[-2,2](-2 前胸,+2 背)。
        float minX = Float.MAX_VALUE, maxX = -Float.MAX_VALUE;
        float minY = Float.MAX_VALUE, maxY = -Float.MAX_VALUE;
        float minZ = Float.MAX_VALUE, maxZ = -Float.MAX_VALUE;
        for (WornPackModel.GeoCube c : WornPackModel.GRASS_POUCH_BACK_CUBES) {
            float yTop = WornPackModel.bedrockToVanillaCuboidY(c.oy(), c.sy());
            float yBot = yTop + c.sy();
            minX = Math.min(minX, c.ox());
            maxX = Math.max(maxX, c.ox() + c.sx());
            minY = Math.min(minY, yTop);
            maxY = Math.max(maxY, yBot);
            minZ = Math.min(minZ, c.oz());
            maxZ = Math.max(maxZ, c.oz() + c.sz());
        }

        // 比例：背包件不应宽过躯干（body 半宽=4），否则会从肩膀两侧穿出。
        assertTrue(Math.max(Math.abs(minX), Math.abs(maxX)) <= 4.0f,
            "背包件横向半宽应 <= body 半宽 4，实际 |x|max=" + Math.max(Math.abs(minX), Math.abs(maxX)));

        // 挂点(y)：应挂在躯干背后（颈 y=0 之下、不高过肩），底部不应深陷腿部以下。
        assertTrue(minY >= 0.0f,
            "背包件顶部不应高过肩(y<0)，实际 minY=" + minY + "（若 y 翻转缺失会得 minY≈11.5）");
        assertTrue(maxY <= 13.0f,
            "背包件底部不应深陷腿部以下(y>13)，实际 maxY=" + maxY + "（若 y 翻转缺失会得 maxY≈17.5）");
        assertTrue(minY >= 5.0f,
            "破草包挂在腰背(下-中段)，顶部应在背中部 y>=5，实际 minY=" + minY);

        // 不穿模(z)：应落在躯干背面(z>=躯干背 ~2 附近，允许 <=1px 贴合嵌入)且不浮在身后过远。
        assertTrue(minZ >= 1.0f,
            "背包件不应陷入躯干前侧(z<1)，实际 minZ=" + minZ);
        assertTrue(maxZ <= 6.0f,
            "背包件不应浮在身后过远(z>6)，实际 maxZ=" + maxZ);
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
