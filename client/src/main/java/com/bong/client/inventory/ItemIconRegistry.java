package com.bong.client.inventory;

import com.bong.client.armor.ArmorTintRegistry;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.Identifier;
import net.minecraft.util.InvalidIdentifierException;

import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class ItemIconRegistry {
    public static final String ITEM_TEXTURE_PREFIX = "bong-client:textures/gui/items/";
    public static final String TOOL_TEXTURE_PREFIX = ITEM_TEXTURE_PREFIX + "tools/";
    public static final String FALLBACK_ITEM_PATH = ITEM_TEXTURE_PREFIX + "broken_artifact.png";
    public static final String FALLBACK_SCROLL_PATH = ITEM_TEXTURE_PREFIX + "broken_artifact_scroll.png";

    private static final Identifier FALLBACK_ITEM_TEXTURE = id(FALLBACK_ITEM_PATH);
    private static final Identifier FALLBACK_SCROLL_TEXTURE = id(FALLBACK_SCROLL_PATH);
    private static final Map<String, Identifier> TEXTURE_CACHE = new ConcurrentHashMap<>();
    private static final Map<String, String> GATHERING_TOOL_ICON_PATHS = Map.ofEntries(
        Map.entry("axe_bone", TOOL_TEXTURE_PREFIX + "axe_bone.png"),
        Map.entry("axe_iron", TOOL_TEXTURE_PREFIX + "axe_iron.png"),
        Map.entry("axe_copper", TOOL_TEXTURE_PREFIX + "axe_copper.png"),
        Map.entry("pickaxe_bone", TOOL_TEXTURE_PREFIX + "pickaxe_bone.png"),
        Map.entry("pickaxe_iron", TOOL_TEXTURE_PREFIX + "pickaxe_iron.png"),
        Map.entry("pickaxe_copper", TOOL_TEXTURE_PREFIX + "pickaxe_copper.png")
    );

    public static final Map<String, String> PLANT_ICON_PATHS = Map.ofEntries(
        Map.entry("ci_she_hao", "bong-client:textures/gui/botany/ci_she_hao.png"),
        Map.entry("ning_mai_cao", "bong-client:textures/gui/botany/ning_mai_cao.png"),
        Map.entry("hui_yuan_zhi", "bong-client:textures/gui/botany/hui_yuan_zhi.png"),
        Map.entry("chi_sui_cao", "bong-client:textures/gui/botany/chi_sui_cao.png"),
        Map.entry("gu_yuan_gen", "bong-client:textures/gui/botany/gu_yuan_gen.png"),
        Map.entry("kong_shou_hen", "bong-client:textures/gui/botany/kong_shou_hen.png"),
        Map.entry("jie_gu_rui", "bong-client:textures/gui/botany/jie_gu_rui.png"),
        Map.entry("yang_jing_tai", "bong-client:textures/gui/botany/yang_jing_tai.png"),
        Map.entry("qing_zhuo_cao", "bong-client:textures/gui/botany/qing_zhuo_cao.png"),
        Map.entry("an_shen_guo", "bong-client:textures/gui/botany/an_shen_guo.png"),
        Map.entry("shi_mai_gen", "bong-client:textures/gui/botany/shi_mai_gen.png"),
        Map.entry("ling_yan_shi_zhi", "bong-client:textures/gui/botany/ling_yan_shi_zhi.png"),
        Map.entry("ye_ku_teng", "bong-client:textures/gui/botany/ye_ku_teng.png"),
        Map.entry("hui_jin_tai", "bong-client:textures/gui/botany/hui_jin_tai.png"),
        Map.entry("zhen_jie_zi", "bong-client:textures/gui/botany/zhen_jie_zi.png"),
        Map.entry("shao_hou_man", "bong-client:textures/gui/botany/shao_hou_man.png"),
        Map.entry("tian_nu_jiao", "bong-client:textures/gui/botany/tian_nu_jiao.png"),
        Map.entry("fu_you_hua", "bong-client:textures/gui/botany/fu_you_hua.png"),
        Map.entry("wu_yan_guo", "bong-client:textures/gui/botany/wu_yan_guo.png"),
        Map.entry("hei_gu_jun", "bong-client:textures/gui/botany/hei_gu_jun.png"),
        Map.entry("fu_chen_cao", "bong-client:textures/gui/botany/fu_chen_cao.png"),
        Map.entry("zhong_yan_teng", "bong-client:textures/gui/botany/zhong_yan_teng.png"),
        Map.entry("fu_yuan_jue", itemTexturePath("fu_yuan_jue")),
        Map.entry("bai_yan_peng", itemTexturePath("bai_yan_peng")),
        Map.entry("duan_ji_ci", itemTexturePath("duan_ji_ci")),
        Map.entry("xue_se_mai_cao", itemTexturePath("xue_se_mai_cao")),
        Map.entry("yun_ding_lan", itemTexturePath("yun_ding_lan")),
        Map.entry("xuan_gen_wei", itemTexturePath("xuan_gen_wei")),
        Map.entry("ying_yuan_gu", itemTexturePath("ying_yuan_gu")),
        Map.entry("xuan_rong_tai", itemTexturePath("xuan_rong_tai")),
        Map.entry("yuan_ni_hong_yu", itemTexturePath("yuan_ni_hong_yu")),
        Map.entry("jing_xin_zao", itemTexturePath("jing_xin_zao")),
        Map.entry("xue_po_lian", itemTexturePath("xue_po_lian")),
        Map.entry("jiao_mai_teng", itemTexturePath("jiao_mai_teng")),
        Map.entry("lie_yuan_tai", itemTexturePath("lie_yuan_tai")),
        Map.entry("ming_gu_gu", itemTexturePath("ming_gu_gu")),
        Map.entry("bei_wen_zhi", itemTexturePath("bei_wen_zhi")),
        Map.entry("ling_jing_xu", itemTexturePath("ling_jing_xu")),
        Map.entry("mao_xin_wei", itemTexturePath("mao_xin_wei"))
    );

    private ItemIconRegistry() {}

    public static Identifier textureIdForItemId(String itemId) {
        String normalized = normalize(itemId);
        return TEXTURE_CACHE.computeIfAbsent(normalized, ItemIconRegistry::resolveTextureIdForItemId);
    }

    public static String itemTexturePath(String itemId) {
        String normalized = normalize(itemId);
        return GATHERING_TOOL_ICON_PATHS.getOrDefault(normalized, ITEM_TEXTURE_PREFIX + normalized + ".png");
    }

    public static String plantIconPath(String plantKindId) {
        return PLANT_ICON_PATHS.get(normalize(plantKindId));
    }

    public static Identifier fallbackTextureIdForItemId(String itemId) {
        return isScrollTextureCandidate(itemId) ? FALLBACK_SCROLL_TEXTURE : FALLBACK_ITEM_TEXTURE;
    }

    public static boolean isScrollTextureCandidate(String itemId) {
        String normalized = normalize(itemId);
        return normalized.startsWith("skill_scroll_")
            || normalized.startsWith("recipe_scroll_")
            || normalized.startsWith("blueprint_scroll_")
            || normalized.startsWith("inscription_scroll_")
            || normalized.endsWith("_scroll");
    }

    private static Identifier resolveTextureIdForItemId(String itemId) {
        if (itemId.isEmpty()) {
            return FALLBACK_ITEM_TEXTURE;
        }

        Identifier candidate;
        try {
            String armorPath = ArmorTintRegistry.iconPathForItemId(itemId);
            candidate = id(armorPath == null ? itemTexturePath(itemId) : armorPath);
        } catch (InvalidIdentifierException exception) {
            return fallbackTextureIdForItemId(itemId);
        }
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.getResourceManager().getResource(candidate).isPresent()) {
            return candidate;
        }
        return fallbackTextureIdForItemId(itemId);
    }

    private static Identifier id(String path) {
        return new Identifier(path);
    }

    private static String normalize(String itemId) {
        return itemId == null ? "" : itemId.trim().toLowerCase(Locale.ROOT);
    }
}
