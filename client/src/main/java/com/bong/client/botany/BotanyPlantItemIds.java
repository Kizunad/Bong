package com.bong.client.botany;

import java.util.Set;

public final class BotanyPlantItemIds {
    private static final Set<String> IDS = Set.of(
        "ci_she_hao",
        "ning_mai_cao",
        "hui_yuan_zhi",
        "chi_sui_cao",
        "gu_yuan_gen",
        "kong_shou_hen",
        "jie_gu_rui",
        "yang_jing_tai",
        "qing_zhuo_cao",
        "an_shen_guo",
        "shi_mai_gen",
        "ling_yan_shi_zhi",
        "ye_ku_teng",
        "hui_jin_tai",
        "zhen_jie_zi",
        "shao_hou_man",
        "tian_nu_jiao",
        "fu_you_hua",
        "wu_yan_guo",
        "hei_gu_jun",
        "fu_chen_cao",
        "zhong_yan_teng",
        "fu_yuan_jue",
        "bai_yan_peng",
        "duan_ji_ci",
        "xue_se_mai_cao",
        "yun_ding_lan",
        "xuan_gen_wei",
        "ying_yuan_gu",
        "xuan_rong_tai",
        "yuan_ni_hong_yu",
        "jing_xin_zao",
        "xue_po_lian",
        "jiao_mai_teng",
        "lie_yuan_tai",
        "ming_gu_gu",
        "bei_wen_zhi",
        "ling_jing_xu",
        "mao_xin_wei"
    );

    private BotanyPlantItemIds() {}

    public static boolean contains(String itemId) {
        return itemId != null && IDS.contains(itemId.trim());
    }
}
