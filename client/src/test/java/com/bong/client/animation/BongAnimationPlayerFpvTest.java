package com.bong.client.animation;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.kosmx.playerAnim.api.firstPerson.FirstPersonConfiguration;
import dev.kosmx.playerAnim.api.firstPerson.FirstPersonMode;
import dev.kosmx.playerAnim.api.layered.KeyframeAnimationPlayer;
import dev.kosmx.playerAnim.core.data.AnimationFormat;
import dev.kosmx.playerAnim.core.data.KeyframeAnimation;
import net.minecraft.util.Identifier;
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
}
