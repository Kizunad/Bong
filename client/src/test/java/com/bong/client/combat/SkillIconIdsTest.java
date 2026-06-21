package com.bong.client.combat;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * {@link SkillIconIds} 的 id→文件名→贴图路径规约。
 *
 * <p>HUD 与查看界面共用这套规约，必须保证同一技能 id 在两端解析到同一路径，
 * 否则又会出现「一端有图、一端紫黑」。这里把 {@code '.'/':'/'/' } 规整与路径拼接逐条锁死。</p>
 */
class SkillIconIdsTest {

    @Test
    void dotsBecomeUnderscores() {
        assertEquals("woliu_heart", SkillIconIds.safeSkillIconId("woliu.heart"),
            "技能 id 的 '.' 应规整为 '_'");
    }

    @Test
    void colonsAndSlashesBecomeUnderscores() {
        assertEquals("a_b_c_d", SkillIconIds.safeSkillIconId("a:b/c.d"),
            "':'、'/'、'.' 都应规整为 '_'");
    }

    @Test
    void plainIdUnchanged() {
        assertEquals("zhenmai_harden", SkillIconIds.safeSkillIconId("zhenmai_harden"),
            "已是安全字符的 id 原样返回");
    }

    @Test
    void nullOrBlankBecomesUnknown() {
        assertEquals("unknown", SkillIconIds.safeSkillIconId(null), "null → unknown");
        assertEquals("unknown", SkillIconIds.safeSkillIconId(""), "空串 → unknown");
        assertEquals("unknown", SkillIconIds.safeSkillIconId("   "), "纯空白 → unknown");
    }

    @Test
    void scrollItemIdPrefixesSafeId() {
        assertEquals("skill_scroll_woliu_heart", SkillIconIds.scrollItemId("woliu.heart"),
            "itemId = skill_scroll_ + 安全 id");
    }

    @Test
    void scrollTexturePathIsFullIdentifier() {
        assertEquals(
            "bong-client:textures/gui/items/skill_scroll_woliu_heart.png",
            SkillIconIds.scrollTexturePath("woliu.heart"),
            "贴图路径 = 前缀 + skill_scroll_<安全 id> + .png，须与 ItemIconRegistry 解析一致");
    }

    @Test
    void scrollTexturePathHandlesNullId() {
        assertEquals(
            "bong-client:textures/gui/items/skill_scroll_unknown.png",
            SkillIconIds.scrollTexturePath(null),
            "null id 不抛异常，落到 skill_scroll_unknown.png");
    }
}
