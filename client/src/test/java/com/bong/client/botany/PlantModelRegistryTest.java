package com.bong.client.botany;

import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import software.bernie.geckolib.cache.object.GeoBone;
import software.bernie.geckolib.loading.json.raw.Model;
import software.bernie.geckolib.loading.object.BakedModelFactory;
import software.bernie.geckolib.loading.object.GeometryTree;
import software.bernie.geckolib.util.JsonUtil;

import javax.imageio.ImageIO;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class PlantModelRegistryTest {
    private static final List<String> MODELED_PLANTS = List.of(
        "chi_sui_cao", "gu_yuan_gen", "hei_gu_jun", "ling_yan_shi_zhi", "xue_po_lian",
        "tui_gu_teng", "shou_xin_cao", "long_lin_tai", "xu_yuan_rui", "hua_xing_gen",
        "hui_jin_tai", "lie_yuan_tai", "shi_ling_xian", "xuan_rong_tai", "yang_jing_tai",
        "jing_xin_zao");

    @AfterEach
    void clearCatalog() {
        PlantModelRegistry.loadForTest("{}");
    }

    @Test
    void packagedCatalogResolvesModelsAndKeepsExistingGrowthStagesAndAliases() throws Exception {
        loadPackagedCatalog();
        assertTrue(PlantModelRegistry.geoPlantIds().containsAll(MODELED_PLANTS), "这批作者模型必须全部可查询");
        assertSame(PlantModelRegistry.stage("chi_sui_cao", PlantGrowthStage.MATURE).orElseThrow(),
            PlantModelRegistry.stage("xue_cao", PlantGrowthStage.MATURE).orElseThrow());
        for (String id : List.of("chi_sui_cao", "gu_yuan_gen", "hei_gu_jun", "ling_yan_shi_zhi", "xue_po_lian")) {
            assertTrue(PlantModelRegistry.stage(id, PlantGrowthStage.MATURE).orElseThrow().isGeo(), id);
            for (var stage : List.of(PlantGrowthStage.SEEDLING, PlantGrowthStage.GROWING)) {
                var model = PlantModelRegistry.stage(id, stage).orElseThrow();
                assertFalse(model.isGeo(), "已有幼苗和生长阶段不应被成熟模型覆盖：" + id);
                try (var stream = resource(model.texture())) {
                    assertNotNull(ImageIO.read(stream), "阶段贴图必须能解码：" + id);
                }
            }
        }
        assertTrue(PlantModelRegistry.stage("missing_plant", PlantGrowthStage.MATURE).isEmpty());
        assertFalse(PlantModelRegistry.stage("jiao_mai_teng", PlantGrowthStage.MATURE).orElseThrow().isGeo());
    }

    @Test
    void packagedGeometryBakesAndMatchesTextureAtlas() throws Exception {
        loadPackagedCatalog();
        for (String id : PlantModelRegistry.geoPlantIds()) {
            var stage = PlantModelRegistry.stage(id, PlantGrowthStage.MATURE).orElseThrow();
            try (var geometry = resource(stage.geometry()); var texture = resource(stage.texture())) {
                var parsed = JsonUtil.GEO_GSON.fromJson(new String(geometry.readAllBytes(), StandardCharsets.UTF_8), Model.class);
                var tree = GeometryTree.fromModel(parsed);
                var baked = BakedModelFactory.getForNamespace(stage.geometry().getNamespace()).constructGeoModel(tree);
                assertTrue(baked.topLevelBones().stream().mapToInt(PlantModelRegistryTest::cubeCount).sum() > 0,
                    "模型必须能被 GeckoLib 烘焙为非空几何：" + id);
                var image = ImageIO.read(texture);
                assertNotNull(image, "图集必须是有效图片：" + id);
                assertEquals((int) tree.properties().textureWidth(), image.getWidth(), id);
                assertEquals((int) tree.properties().textureHeight(), image.getHeight(), id);
            }
        }
    }

    @Test
    void optionalAnimationAndMalformedGeometryDoNotTurnIntoBillboards() throws Exception {
        var catalog = JsonParser.parseString(catalogJson()).getAsJsonObject();
        var plant = catalog.getAsJsonArray("plants").get(0).getAsJsonObject();
        var mature = plant.getAsJsonObject("visual").getAsJsonObject("stages").getAsJsonObject("mature");
        mature.addProperty("kind", "geo");
        mature.addProperty("geometry", "bong:geo/plants/example.geo.json");
        mature.add("animation", JsonParser.parseString("""
            {"resource":"bong:animations/plants/example.animation.json","idle":"animation.example.idle"}
            """));
        PlantModelRegistry.loadForTest(catalog.toString());
        var stage = PlantModelRegistry.stage(plant.get("id").getAsString(), PlantGrowthStage.MATURE).orElseThrow();
        assertTrue(stage.isGeo());
        assertEquals("animation.example.idle", stage.animation().idle());

        mature.addProperty("geometry", "bong:geo/plants/example.png");
        PlantModelRegistry.loadForTest(catalog.toString());
        assertTrue(PlantModelRegistry.stage(plant.get("id").getAsString(), PlantGrowthStage.MATURE).isEmpty(),
            "非法 geo 配置不可把图集当作整张植物贴图绘制");
        assertTrue(PlantModelRegistry.geoPlantIds().contains("chi_sui_cao"), "单个错误条目不能影响其他植物");
    }

    private static void loadPackagedCatalog() throws Exception {
        PlantModelRegistry.loadForTest(catalogJson());
    }

    private static String catalogJson() throws Exception {
        try (var stream = resource(new Identifier("bong-client", "botany/plants.json"))) {
            return new String(stream.readAllBytes(), StandardCharsets.UTF_8);
        }
    }

    private static InputStream resource(Identifier id) {
        var stream = PlantModelRegistryTest.class.getResourceAsStream("/assets/" + id.getNamespace() + "/" + id.getPath());
        assertNotNull(stream, "客户端包缺少资源：" + id);
        return stream;
    }

    private static int cubeCount(GeoBone bone) {
        return bone.getCubes().size() + bone.getChildBones().stream().mapToInt(PlantModelRegistryTest::cubeCount).sum();
    }
}
