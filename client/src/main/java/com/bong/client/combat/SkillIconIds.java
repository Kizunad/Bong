package com.bong.client.combat;

/**
 * 技能/功法图标的 id 与贴图路径统一规约。
 *
 * <p>HUD（{@code LoadoutIconLayer}）与查看界面（{@code InspectScreen}/{@code ItemIconRegistry}）共用，
 * 避免两端各拼一套导致同一功法在两处解析到不同路径（紫黑 missing-texture 的根因之一就是两条路径不一致）。</p>
 *
 * <p>约定：一张 {@code assets/bong-client/textures/gui/items/skill_scroll_<safeId>.png} 同时被
 * HUD 与查看界面命中。前缀与 {@code ItemIconRegistry.ITEM_TEXTURE_PREFIX} 保持一致。</p>
 */
public final class SkillIconIds {
    /** 与 {@code ItemIconRegistry.ITEM_TEXTURE_PREFIX} 一致；改动需两处同步。 */
    private static final String ITEM_TEXTURE_PREFIX = "bong-client:textures/gui/items/";

    private SkillIconIds() {
    }

    /** 把技能 id 规整成可作文件名的安全 id（{@code '.'/':'/'/' → '_'}）；空 → {@code "unknown"}。 */
    public static String safeSkillIconId(String skillId) {
        if (skillId == null || skillId.isBlank()) {
            return "unknown";
        }
        return skillId.replace('.', '_').replace(':', '_').replace('/', '_');
    }

    /** 查看界面用的伪 itemId：{@code skill_scroll_<safeId>}（交给 ItemIconRegistry 解析）。 */
    public static String scrollItemId(String skillId) {
        return "skill_scroll_" + safeSkillIconId(skillId);
    }

    /** HUD 用的完整贴图 Identifier 字符串：{@code bong-client:textures/gui/items/skill_scroll_<safeId>.png}。 */
    public static String scrollTexturePath(String skillId) {
        return ITEM_TEXTURE_PREFIX + scrollItemId(skillId) + ".png";
    }
}
