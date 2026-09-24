package com.bong.client.animation;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.kosmx.playerAnim.api.firstPerson.FirstPersonConfiguration;
import dev.kosmx.playerAnim.api.firstPerson.FirstPersonMode;
import dev.kosmx.playerAnim.api.layered.KeyframeAnimationPlayer;
import dev.kosmx.playerAnim.core.data.AnimationFormat;
import dev.kosmx.playerAnim.core.data.KeyframeAnimation;
import java.util.UUID;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

/**
 * plan-fpv-cast-av-v1 P1：锁本地玩家 FPV 变体查找的两个纯单元——变体 id 派生 + 第一人称
 * 渲染配置（路线 A，§8.1 #1）。本地玩家判定（{@code isLocalPlayer}）依赖运行中的
 * {@code MinecraftClient}，走真机验收（P0 已真机拍板路线 A）；此处锁可纯测的确定性逻辑。
 */
class BongAnimationPlayerFpvTest {

    private static KeyframeAnimationPlayer freshPlayer() {
        KeyframeAnimation.AnimationBuilder builder =
            new KeyframeAnimation.AnimationBuilder(AnimationFormat.UNKNOWN);
        builder.endTick = 1;
        builder.isLooped = false;
        return new KeyframeAnimationPlayer(builder.build());
    }

    /** 最小合法 emotecraft v3 JSON——只为让 {@code BongAnimationRegistry.contains} 命中变体。 */
    private static String inlineJson(String name) {
        return "{\"version\":3,\"name\":\"" + name + "\",\"emote\":{"
            + "\"beginTick\":0,\"endTick\":4,\"isLoop\":false,\"moves\":["
            + "{\"tick\":0,\"rightArm\":{\"pitch\":-0.6},\"easing\":\"LINEAR\"},"
            + "{\"tick\":4,\"rightArm\":{\"pitch\":0.4},\"easing\":\"INOUTSINE\"}"
            + "]}}";
    }

    @AfterEach
    void tearDown() {
        BongAnimationRegistry.clearInlineForTest();
        BongAnimationPlayer.resetForTest();  // 复位注入的本地玩家判定 seam
    }

    @Test
    void fpvVariantIdAppendsSuffixPreservingNamespace() {
        Identifier base = new Identifier("bong", "sword_cleave");
        Identifier fpv = BongAnimationPlayer.fpvVariantId(base);
        assertEquals("bong", fpv.getNamespace(), "命名空间必须保留");
        assertEquals("sword_cleave_fpv", fpv.getPath(),
            "变体 id = 原 path + _fpv（对应资源 player_animation/sword_cleave_fpv.json）");
    }

    @Test
    void fpvArmsOn_thirdPersonModel_allArmsAndItemsShown() {
        KeyframeAnimationPlayer p = freshPlayer();
        BongAnimationPlayer.applyFirstPersonRendering(p, true);
        assertEquals(
            FirstPersonMode.THIRD_PERSON_MODEL, p.getFirstPersonMode(0f),
            "路线 A 用 THIRD_PERSON_MODEL（库原生 FP 渲染 + ItemInHandRendererMixin 掐 vanilla FP）");
        FirstPersonConfiguration cfg = p.getFirstPersonConfiguration(0f);
        assertTrue(cfg.isShowRightArm(), "本地玩家 + 有 _fpv 变体：右臂必须可见");
        assertTrue(cfg.isShowLeftArm(), "左臂必须可见（双手持剑）");
        assertTrue(cfg.isShowRightItem(), "右手持物可见");
        assertTrue(cfg.isShowLeftItem(), "左手持物可见");
    }

    @Test
    void fpvArmsOff_reproducesShippedBehavior_armsHidden() {
        KeyframeAnimationPlayer p = freshPlayer();
        BongAnimationPlayer.applyFirstPersonRendering(p, false);
        assertEquals(
            FirstPersonMode.THIRD_PERSON_MODEL, p.getFirstPersonMode(0f),
            "无变体 / 远端玩家仍走 THIRD_PERSON_MODEL——保持出厂第一人称渲染路径");
        FirstPersonConfiguration cfg = p.getFirstPersonConfiguration(0f);
        assertFalse(cfg.isShowRightArm(),
            "库默认 config showRightArm=false——这正是出厂第一人称隐藏手臂的行为，须保持不变");
        assertFalse(cfg.isShowLeftArm(), "默认 showLeftArm=false");
    }

    // resolveFpvContent 的分支覆盖（finder 抓的缺口）：过去这条 wiring 只在真机验证，
    // headless 下 MinecraftClient 恒 null 无法驱动。现经 localPlayerPredicate seam 注入，
    // 把「本地+有变体→取变体开双臂 / 本地+无变体→取原招 / 远端+有变体→永不取 FPV」
    // 三条 plan §P1 硬约束纳入自动化回归。

    @Test
    void resolveFpvContent_localPlayerWithVariant_picksVariantAndArms() {
        UUID pid = UUID.randomUUID();
        Identifier base = new Identifier("bong", "unit_cleave");
        Identifier variant = BongAnimationPlayer.fpvVariantId(base);  // bong:unit_cleave_fpv
        assertTrue(BongAnimationRegistry.registerInlineJson(variant, inlineJson("unit_cleave_fpv")),
            "前置：变体必须成功注册，contains 才会命中");
        BongAnimationPlayer.setLocalPlayerPredicateForTest(pid::equals);

        BongAnimationPlayer.FpvResolution r = BongAnimationPlayer.resolveFpvContent(pid, base);
        assertEquals(variant, r.contentId(),
            "本地玩家 + 存在 _fpv 变体 → 必须取变体 id（贴脸专调姿态），实际取了 " + r.contentId());
        assertTrue(r.useFpvArms(), "命中变体必须开第一人称双臂（路线 A）");
    }

    @Test
    void resolveFpvContent_localPlayerNoVariant_picksBaseNoArms() {
        UUID pid = UUID.randomUUID();
        Identifier base = new Identifier("bong", "unit_no_variant");
        // 不注册 <base>_fpv 变体
        BongAnimationPlayer.setLocalPlayerPredicateForTest(pid::equals);

        BongAnimationPlayer.FpvResolution r = BongAnimationPlayer.resolveFpvContent(pid, base);
        assertEquals(base, r.contentId(),
            "本地玩家但无 _fpv 变体 → 必须回落原招（出厂 TPV 动画），实际取了 " + r.contentId());
        assertFalse(r.useFpvArms(), "无变体不得开第一人称双臂（否则 TPV 姿态在贴脸视角穿帮）");
    }

    @Test
    void resolveFpvContent_remotePlayerWithVariant_neverPicksFpv() {
        UUID localPid = UUID.randomUUID();
        UUID remotePid = UUID.randomUUID();
        Identifier base = new Identifier("bong", "unit_cleave");
        Identifier variant = BongAnimationPlayer.fpvVariantId(base);
        assertTrue(BongAnimationRegistry.registerInlineJson(variant, inlineJson("unit_cleave_fpv")),
            "前置：即便变体存在，远端玩家也不得取用");
        // 只有 localPid 是本地玩家；查询 remotePid
        BongAnimationPlayer.setLocalPlayerPredicateForTest(localPid::equals);

        BongAnimationPlayer.FpvResolution r = BongAnimationPlayer.resolveFpvContent(remotePid, base);
        assertEquals(base, r.contentId(),
            "远端玩家即便有 _fpv 变体也永不取 FPV（plan §P1 硬约束：FPV 只影响本地玩家），"
                + "实际取了 " + r.contentId());
        assertFalse(r.useFpvArms(), "远端玩家不得开第一人称双臂（远端渲染分支零变化）");
    }
}
