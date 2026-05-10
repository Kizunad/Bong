package com.bong.client.inventory;

import org.junit.jupiter.api.Test;

import javax.imageio.ImageIO;
import java.util.List;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class GeneratedItemIconAssetsTest {
    private static final Path ITEM_TEXTURE_DIR = Path.of(
        "src", "main", "resources", "assets", "bong-client", "textures", "gui", "items"
    );
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

    @Test
    void firstBatchGeneratedIconsAre128RgbaPngs() throws Exception {
        for (String itemId : FIRST_BATCH_IDS) {
            Path path = ITEM_TEXTURE_DIR.resolve(itemId + ".png");
            assertTrue(java.nio.file.Files.exists(path), itemId + " icon should exist");
            var image = ImageIO.read(path.toFile());
            assertNotNull(image, itemId + " icon should be readable");
            assertEquals(128, image.getWidth(), itemId + " icon width");
            assertEquals(128, image.getHeight(), itemId + " icon height");
            assertTrue(image.getColorModel().hasAlpha(), itemId + " icon should keep transparent alpha");
        }
    }
}
