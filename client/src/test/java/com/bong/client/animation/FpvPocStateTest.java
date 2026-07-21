package com.bong.client.animation;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.kosmx.playerAnim.api.firstPerson.FirstPersonConfiguration;
import dev.kosmx.playerAnim.api.firstPerson.FirstPersonMode;
import dev.kosmx.playerAnim.api.layered.KeyframeAnimationPlayer;
import dev.kosmx.playerAnim.core.data.AnimationFormat;
import dev.kosmx.playerAnim.core.data.KeyframeAnimation;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

/**
 * plan-fpv-cast-av-v1 P0：锁 {@link FpvPocState} 的路线→(FirstPersonMode + FirstPersonConfiguration)
 * 映射契约与循环序。核心是 <b>OFF 必须等价出厂行为</b>（THIRD_PERSON_MODEL + 默认 config，
 * 第一人称隐藏手臂），否则本 POC 分支会悄悄改变已发布路径。
 */
class FpvPocStateTest {

    @AfterEach
    void reset() {
        FpvPocState.set(FpvPocState.OFF);
    }

    private static KeyframeAnimationPlayer freshPlayer() {
        KeyframeAnimation.AnimationBuilder builder =
            new KeyframeAnimation.AnimationBuilder(AnimationFormat.UNKNOWN);
        builder.endTick = 1;
        builder.isLooped = false;
        return new KeyframeAnimationPlayer(builder.build());
    }

    @Test
    void offReproducesShippedBehavior_thirdPersonModel_armsHidden() {
        KeyframeAnimationPlayer p = freshPlayer();
        FpvPocState.OFF.applyTo(p);
        assertEquals(
            FirstPersonMode.THIRD_PERSON_MODEL, p.getFirstPersonMode(0f),
            "OFF 必须保持出厂 THIRD_PERSON_MODEL——否则第一人称渲染路径被 POC 悄悄改变");
        FirstPersonConfiguration cfg = p.getFirstPersonConfiguration(0f);
        assertFalse(cfg.isShowRightArm(),
            "OFF 用库默认 config：showRightArm=false（这正是出厂第一人称看不到手臂的根因）");
        assertFalse(cfg.isShowLeftArm(), "OFF：showLeftArm 默认 false");
    }

    @Test
    void routeA_libraryNative_thirdPersonModel_armsAndItemsShown() {
        KeyframeAnimationPlayer p = freshPlayer();
        FpvPocState.A.applyTo(p);
        assertEquals(
            FirstPersonMode.THIRD_PERSON_MODEL, p.getFirstPersonMode(0f),
            "路线 A 仍走 THIRD_PERSON_MODEL（库原生 FP 渲染路径），差别在开手臂 config");
        FirstPersonConfiguration cfg = p.getFirstPersonConfiguration(0f);
        assertTrue(cfg.isShowRightArm(), "A：右臂必须可见");
        assertTrue(cfg.isShowLeftArm(), "A：左臂必须可见（双手持剑）");
        assertTrue(cfg.isShowRightItem(), "A：右手持物可见");
        assertTrue(cfg.isShowLeftItem(), "A：左手持物可见");
    }

    @Test
    void routeB_usesNone_leavesFpToOwnLayer() {
        KeyframeAnimationPlayer p = freshPlayer();
        FpvPocState.B.applyTo(p);
        assertEquals(
            FirstPersonMode.NONE, p.getFirstPersonMode(0f),
            "路线 B：库在 FP 透明（NONE），第一人称手臂交给自绘层（渲染器 P0 待补）");
    }

    @Test
    void routeC_usesVanilla_forBoneInjection() {
        KeyframeAnimationPlayer p = freshPlayer();
        FpvPocState.C.applyTo(p);
        assertEquals(
            FirstPersonMode.VANILLA, p.getFirstPersonMode(0f),
            "路线 C：走 vanilla FP 手臂（VANILLA），骨骼变换注入 mixin P0 待补");
    }

    @Test
    void cycleWrapsThroughAllFourRoutesInOrder() {
        FpvPocState.set(FpvPocState.OFF);
        assertEquals(FpvPocState.A, FpvPocState.cycle(), "OFF → A");
        assertEquals(FpvPocState.B, FpvPocState.cycle(), "A → B");
        assertEquals(FpvPocState.C, FpvPocState.cycle(), "B → C");
        assertEquals(FpvPocState.OFF, FpvPocState.cycle(), "C → 回 OFF");
    }

    @Test
    void currentDefaultsToOff() {
        FpvPocState.set(null); // 空值不改变现状（防御）
        assertEquals(FpvPocState.OFF, FpvPocState.current(),
            "set(null) 不应改变当前路线；默认应为 OFF（出厂行为）");
    }
}
