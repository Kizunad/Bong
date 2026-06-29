package com.bong.client.visual.particle;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * F.2 验证：bong:sword_intent_hit 正确注册并路由到 SwordPathVfxPlayer。
 *
 * <p>契约保证（sword-path-complete §F.2）：server C 模块 {@code sword_intent_tracking_system} emit
 * {@code bong:sword_intent_hit} 事件；client 必须通过 EVENT_IDS for-loop 注册到 SwordPathVfxPlayer，
 * 否则剑意命中时粒子静默丢失（VfxParticleBridge bridgeMiss）。
 */
class SwordIntentHitVfxRegistrationTest {

    @AfterEach
    void clearRegistry() {
        VfxRegistry.instance().clearForTests();
    }

    // ─── EVENT_IDS 成员验证 ───────────────────────────────────────────────

    @Test
    void swordIntentHitIsInEventIds() {
        Identifier expected = new Identifier("bong", "sword_intent_hit");
        assertTrue(SwordPathVfxPlayer.EVENT_IDS.contains(expected),
            "SWORD_INTENT_HIT (bong:sword_intent_hit) 必须在 SwordPathVfxPlayer.EVENT_IDS 中；"
                + "server C 模块 emit 此 event_id，未注册则剑意命中 VFX 静默丢失。"
                + "实际 EVENT_IDS=" + SwordPathVfxPlayer.EVENT_IDS);
    }

    @Test
    void swordIntentHitConstantMatchesExpectedId() {
        assertEquals("bong", SwordPathVfxPlayer.SWORD_INTENT_HIT.getNamespace(),
            "SWORD_INTENT_HIT namespace 必须为 'bong'；实际=" + SwordPathVfxPlayer.SWORD_INTENT_HIT.getNamespace());
        assertEquals("sword_intent_hit", SwordPathVfxPlayer.SWORD_INTENT_HIT.getPath(),
            "SWORD_INTENT_HIT path 必须为 'sword_intent_hit'（与契约 §1.2 共享字符串一致）；"
                + "实际=" + SwordPathVfxPlayer.SWORD_INTENT_HIT.getPath());
    }

    // ─── 注册后路由验证 ───────────────────────────────────────────────────

    @Test
    void swordIntentHitRegistersAfterBootstrap() {
        VfxBootstrap.registerDefaults();
        Identifier eventId = SwordPathVfxPlayer.SWORD_INTENT_HIT;
        assertTrue(VfxRegistry.instance().contains(eventId),
            "VfxBootstrap.registerDefaults() 后 bong:sword_intent_hit 必须已注册；"
                + "缺失则 SwordIntentEntity 命中时无粒子（飞剑扎中敌人无视觉反馈）");
    }

    @Test
    void swordIntentHitRoutesToSwordPathVfxPlayer() {
        VfxBootstrap.registerDefaults();
        Identifier eventId = SwordPathVfxPlayer.SWORD_INTENT_HIT;
        VfxPlayer player = VfxRegistry.instance().lookup(eventId).orElse(null);
        assertNotNull(player,
            "bong:sword_intent_hit 注册后 lookup 应返回非 null；实际=null");
        assertInstanceOf(SwordPathVfxPlayer.class, player,
            "bong:sword_intent_hit 应路由到 SwordPathVfxPlayer（共享同一实例复用 flyingSwordTrailSprites）；"
                + "实际=" + player.getClass().getName());
    }

    // ─── EVENT_IDS 大小契约（防止意外增删） ───────────────────────────────

    @Test
    void eventIdsSizeIsCorrectAfterAddingSwordIntentHit() {
        // 原 20 个 event ids + SWORD_INTENT_HIT = 21。
        // 锁住 size 防止意外增删：若 size 不对说明其他 event 被删掉或多加了重复项。
        int expected = 21;
        assertEquals(expected, SwordPathVfxPlayer.EVENT_IDS.size(),
            "SwordPathVfxPlayer.EVENT_IDS 应包含 21 个事件（原 20 + SWORD_INTENT_HIT）；"
                + "实际=" + SwordPathVfxPlayer.EVENT_IDS.size());
    }

    @Test
    void swordIntentHitConstantIsNotPresentInOtherEventLists() {
        // 确认 SWORD_INTENT_HIT 不与其他 VfxPlayer 的 EVENT_IDS 冲突（防重复注册覆盖）
        // AnqiVfxPlayer、DuguNeedleVfxPlayer 各有自己的 EVENT_IDS。
        Identifier intentHit = SwordPathVfxPlayer.SWORD_INTENT_HIT;
        assertFalse(AnqiVfxPlayer.EVENT_IDS.contains(intentHit),
            "SWORD_INTENT_HIT 不应出现在 AnqiVfxPlayer.EVENT_IDS 中（防重复注册覆盖）");
        assertFalse(DuguNeedleVfxPlayer.EVENT_IDS.contains(intentHit),
            "SWORD_INTENT_HIT 不应出现在 DuguNeedleVfxPlayer.EVENT_IDS 中（防重复注册覆盖）");
    }
}
