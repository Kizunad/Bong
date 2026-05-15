package com.bong.client.inventory;

import org.junit.jupiter.api.Test;

import javax.imageio.ImageIO;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
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

    private static void assertGeneratedIconsAre128RgbaPngs(Path textureDir, List<String> itemIds) throws Exception {
        for (String itemId : itemIds) {
            Path path = textureDir.resolve(itemId + ".png");
            assertTrue(Files.exists(path), itemId + " icon should exist");
            var image = ImageIO.read(path.toFile());
            assertNotNull(image, itemId + " icon should be readable");
            assertEquals(128, image.getWidth(), itemId + " icon width");
            assertEquals(128, image.getHeight(), itemId + " icon height");
            assertTrue(image.getColorModel().hasAlpha(), itemId + " icon should keep alpha channel");
        }
    }
}
