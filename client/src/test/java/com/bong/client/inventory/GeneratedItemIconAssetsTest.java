package com.bong.client.inventory;

import org.junit.jupiter.api.Test;

import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class GeneratedItemIconAssetsTest {
    private static final Path ITEM_TEXTURE_DIR = Path.of(
        "src", "main", "resources", "assets", "bong-client", "textures", "gui", "items"
    );
    private static final Path ARMOR_TEXTURE_DIR = ITEM_TEXTURE_DIR.resolve("armor");
    private static final List<String> FIRST_BATCH_IDS = List.of(
        "bone_coin_5",
        "bone_coin_15",
        "bone_coin_40",
        "shu_gu",
        "zhu_gu",
        "feng_he_gu",
        "yi_shou_gu",
        "bian_yi_hexin",
        "fu_ya_hesui",
        "zhen_shi_chu",
        "xuan_iron",
        "kaimai_dan",
        "ningmai_powder",
        "huiyuan_pill",
        "life_extension_pill",
        "anti_spirit_pressure_pill",
        "hoe_iron",
        "hoe_lingtie",
        "hoe_xuantie",
        "cai_yao_dao",
        "bao_chu",
        "cao_lian",
        "dun_qi_jia",
        "gua_dao",
        "gu_hai_qian",
        "bing_jia_shou_tao",
        "rusted_blade",
        "spirit_sword",
        "skill_scroll_herbalism_baicao_can",
        "skill_scroll_alchemy_danhuo_can",
        "skill_scroll_forging_duantie_can",
        "alchemy_recipe_fragment",
        "blueprint_scroll_iron_sword",
        "blueprint_scroll_qing_feng",
        "blueprint_scroll_ling_feng",
        "inscription_scroll_sharp_v0",
        "inscription_scroll_qi_amplify_v0",
        "array_flag",
        "scattered_qi_pearl",
        "zhen_shi_zhong",
        "zhen_shi_gao",
        "anqi_shanggu_bone",
        "anqi_shanggu_bone_charged"
    );
    private static final List<String> EARLY_FLOW_IDS = List.of(
        "crude_wood",
        "wood_handle",
        "iron_ore",
        "stone_chunk",
        "grass_fiber",
        "clay_pot",
        "iron_ingot",
        "iron_needle",
        "eclipse_needle_iron",
        "poison_decoction_fan",
        "grass_rope",
        "copper_ore"
    );
    private static final List<String> ANQI_BATCH_IDS = List.of(
        "jiet_aning_dan",
        "anqi_bone_chip",
        "anqi_bone_chip_charged",
        "anqi_yibian_shougu",
        "anqi_yibian_shougu_charged",
        "anqi_lingmu_arrow",
        "anqi_lingmu_arrow_charged",
        "anqi_dyed_bone",
        "anqi_dyed_bone_charged",
        "anqi_fenglinghe_bone",
        "anqi_fenglinghe_bone_charged",
        "anqi_container_quiver"
    );

    @Test
    void firstBatchGeneratedIconsAre128RgbaPngs() throws Exception {
        assertGeneratedIconsAre128RgbaPngs(ITEM_TEXTURE_DIR, FIRST_BATCH_IDS);
    }

    @Test
    void earlyFlowGeneratedIconsAre128RgbaPngs() throws Exception {
        assertGeneratedIconsAre128RgbaPngs(ITEM_TEXTURE_DIR, EARLY_FLOW_IDS);
    }

    @Test
    void anqiBatchGeneratedIconsAre128RgbaPngs() throws Exception {
        assertGeneratedIconsAre128RgbaPngs(ITEM_TEXTURE_DIR, ANQI_BATCH_IDS);
    }

    @Test
    void mundaneArmorRepresentativeIconsAre128RgbaPngs() throws Exception {
        assertGeneratedIconsAre128RgbaPngs(ARMOR_TEXTURE_DIR, List.of(
            "armor_bone",
            "armor_hide",
            "armor_iron",
            "armor_copper",
            "armor_spirit_cloth",
            "armor_scroll_wrap"
        ));
    }

    @Test
    void generatedIconAssertionAllowsEmptyList() throws Exception {
        Path dir = Files.createTempDirectory("bong-empty-icons");

        assertGeneratedIconsAre128RgbaPngs(dir, List.of());
    }

    @Test
    void generatedIconAssertionReportsMissingItemId() throws Exception {
        Path dir = Files.createTempDirectory("bong-missing-icons");

        AssertionError error = assertThrows(
            AssertionError.class,
            () -> assertGeneratedIconsAre128RgbaPngs(dir, List.of("missing_icon"))
        );

        assertTrue(error.getMessage().contains("missing_icon"), error.getMessage());
    }

    @Test
    void generatedIconAssertionReportsWrongDimensions() throws Exception {
        Path dir = Files.createTempDirectory("bong-wrong-size-icons");
        writePng(dir.resolve("bad_size.png"), new BufferedImage(64, 128, BufferedImage.TYPE_INT_ARGB));

        AssertionError error = assertThrows(
            AssertionError.class,
            () -> assertGeneratedIconsAre128RgbaPngs(dir, List.of("bad_size"))
        );

        assertTrue(error.getMessage().contains("bad_size"), error.getMessage());
        assertTrue(error.getMessage().contains("actual width=64"), error.getMessage());
    }

    @Test
    void generatedIconAssertionReportsMissingAlpha() throws Exception {
        Path dir = Files.createTempDirectory("bong-rgb-icons");
        writePng(dir.resolve("rgb_icon.png"), new BufferedImage(128, 128, BufferedImage.TYPE_INT_RGB));

        AssertionError error = assertThrows(
            AssertionError.class,
            () -> assertGeneratedIconsAre128RgbaPngs(dir, List.of("rgb_icon"))
        );

        assertTrue(error.getMessage().contains("rgb_icon"), error.getMessage());
        assertTrue(error.getMessage().contains("hasAlpha=false"), error.getMessage());
    }

    private static void assertGeneratedIconsAre128RgbaPngs(Path textureDir, List<String> itemIds) throws Exception {
        for (String itemId : itemIds) {
            Path path = textureDir.resolve(itemId + ".png");
            assertTrue(
                Files.exists(path),
                "expected file exists because icon asset must be generated, actual missing path=" + path + ", itemId=" + itemId
            );
            var image = ImageIO.read(path.toFile());
            assertNotNull(
                image,
                "expected readable PNG because generated icon must be decodable, actual unreadable/null image, path=" + path + ", itemId=" + itemId
            );
            assertEquals(
                128,
                image.getWidth(),
                "expected width=128 because icon spec is 128x128, actual width=" + image.getWidth() + ", itemId=" + itemId
            );
            assertEquals(
                128,
                image.getHeight(),
                "expected height=128 because icon spec is 128x128, actual height=" + image.getHeight() + ", itemId=" + itemId
            );
            assertTrue(
                image.getColorModel().hasAlpha(),
                "expected alpha channel because UI needs transparent background, actual hasAlpha=false, itemId=" + itemId
            );
        }
    }

    private static void writePng(Path path, BufferedImage image) throws Exception {
        assertTrue(ImageIO.write(image, "png", path.toFile()));
    }
}
