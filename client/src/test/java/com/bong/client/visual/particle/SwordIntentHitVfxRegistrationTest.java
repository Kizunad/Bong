package com.bong.client.visual.particle;

import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.texture.Sprite;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.random.Random;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.lang.reflect.Method;

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

    // ─── 外观契约（fallback rgb/count/duration + sprite provider 路由） ────────
    // 这些值是 server sword_intent_tracking_system emit payload 的 fallback 镜像；
    // 真机粒子长相靠它们锁定。私有 static 方法经反射读取（无 getter，同 package 但 private）。

    private static int invokeIntFallback(String methodName, Identifier eventId) throws Exception {
        Method m = SwordPathVfxPlayer.class.getDeclaredMethod(methodName, Identifier.class);
        m.setAccessible(true);
        return (Integer) m.invoke(null, eventId); // auto-unbox → int
    }

    private static SpriteProvider invokeSpriteProvider(Identifier eventId) throws Exception {
        Method m = SwordPathVfxPlayer.class.getDeclaredMethod("spriteProviderFor", Identifier.class);
        m.setAccessible(true);
        return (SpriteProvider) m.invoke(null, eventId);
    }

    @Test
    void swordIntentHitFallbackRgbIsPaleGreenWhite() throws Exception {
        int rgb = invokeIntFallback("fallbackRgb", SwordPathVfxPlayer.SWORD_INTENT_HIT);
        assertEquals(0xE0E8D0, rgb,
            "SWORD_INTENT_HIT fallbackRgb 必须为 0xE0E8D0（淡绿白，飞剑命中灵气余光，"
                + "与 server sword_intent_tracking_system emit 的 color #E0E8D0 一致）；"
                + "实际 0x" + Integer.toHexString(rgb).toUpperCase());
    }

    @Test
    void swordIntentHitFallbackCountIsEight() throws Exception {
        int count = invokeIntFallback("fallbackCount", SwordPathVfxPlayer.SWORD_INTENT_HIT);
        assertEquals(8, count,
            "SWORD_INTENT_HIT fallbackCount 必须为 8（契约锁定值，与 server emit count=8 一致，"
                + "命中那帧迸 8 粒飞剑碎光）；实际 " + count);
    }

    @Test
    void swordIntentHitFallbackDurationIsSixteen() throws Exception {
        int duration = invokeIntFallback("fallbackDuration", SwordPathVfxPlayer.SWORD_INTENT_HIT);
        assertEquals(16, duration,
            "SWORD_INTENT_HIT fallbackDuration 必须为 16t（0.8s 短促命中闪光，"
                + "与 server emit duration_ticks=16 一致）；实际 " + duration);
    }

    @Test
    void swordIntentHitRoutesToFlyingSwordTrailSpriteProvider() throws Exception {
        // spriteProviderFor 返回 BongParticles.flyingSwordTrailSprites（headless 下该 static volatile
        // 为 null）。设两枚 sentinel 区分"飞剑轨迹"分支 vs 默认 swordQiTrail 分支，
        // 锁住 SWORD_INTENT_HIT 与 SWORD_MANIFEST_* 同走飞剑轨迹、不落默认分支。
        SpriteProvider flyingSentinel = new SpriteProvider() {
            @Override
            public Sprite getSprite(int age, int maxAge) {
                return null;
            }

            @Override
            public Sprite getSprite(Random random) {
                return null;
            }
        };
        SpriteProvider defaultSentinel = new SpriteProvider() {
            @Override
            public Sprite getSprite(int age, int maxAge) {
                return null;
            }

            @Override
            public Sprite getSprite(Random random) {
                return null;
            }
        };
        SpriteProvider savedFlying = BongParticles.flyingSwordTrailSprites;
        SpriteProvider savedDefault = BongParticles.swordQiTrailSprites;
        try {
            BongParticles.flyingSwordTrailSprites = flyingSentinel;
            BongParticles.swordQiTrailSprites = defaultSentinel;

            SpriteProvider intentProvider =
                invokeSpriteProvider(SwordPathVfxPlayer.SWORD_INTENT_HIT);
            assertSame(flyingSentinel, intentProvider,
                "SWORD_INTENT_HIT 必须路由到 BongParticles.flyingSwordTrailSprites（飞剑轨迹 sprite），"
                    + "与 SWORD_MANIFEST_SUMMON/STRIKE 同一分支；实际取到的是默认/其他 provider");
            assertSame(flyingSentinel,
                invokeSpriteProvider(SwordPathVfxPlayer.SWORD_MANIFEST_SUMMON),
                "SWORD_MANIFEST_SUMMON 也应走 flyingSwordTrailSprites（确认 SWORD_INTENT_HIT 复用同分支，"
                    + "非独立贴图）");
            assertNotSame(defaultSentinel, intentProvider,
                "SWORD_INTENT_HIT 不应落到默认 swordQiTrailSprites 分支（否则飞剑命中粒子用错贴图）");
        } finally {
            BongParticles.flyingSwordTrailSprites = savedFlying;
            BongParticles.swordQiTrailSprites = savedDefault;
        }
    }
}
