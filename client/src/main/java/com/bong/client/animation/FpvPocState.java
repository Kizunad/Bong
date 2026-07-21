package com.bong.client.animation;

import dev.kosmx.playerAnim.api.firstPerson.FirstPersonConfiguration;
import dev.kosmx.playerAnim.api.firstPerson.FirstPersonMode;
import dev.kosmx.playerAnim.api.layered.KeyframeAnimationPlayer;

/**
 * plan-fpv-cast-av-v1 P0：第一人称手臂动画技术路线 A/B/C POC 的运行时路线选择器。
 *
 * <p>本地玩家动画播放时 {@link BongAnimationPlayer#playOnStack} 调 {@link #applyTo} 按当前
 * route 决定 {@link FirstPersonMode} + {@link FirstPersonConfiguration}，配 {@link FpvPocControls}
 * 键位实时切换，让用户在 runClient 里对比**持物遮挡**（§8 #1 决定性判据只能真机看）。
 *
 * <p><b>关键实证（2026-07-22，读 player-animation-lib 1.0.2-rc1 源码）</b>：该版本
 * {@code FirstPersonMode} 只有 {@code NONE / VANILLA / THIRD_PERSON_MODEL / DISABLED}，
 * <b>没有 plan 预判的 {@code ENABLED}</b>。库原生「第一人称显示动画手臂」的正解是
 * {@code THIRD_PERSON_MODEL} + 一个 {@code showRightArm/showLeftArm=true} 的
 * {@link FirstPersonConfiguration}——而出厂代码只设了 mode、没设 config，默认 config 的
 * {@code showRightArm/showLeftArm=false} 正是第一人称看不到手臂动作的根因。库的
 * {@code ItemInHandRendererMixin} 在 {@code THIRD_PERSON_MODEL} 下会**整段 cancel** vanilla
 * FP 手/物渲染，改由模型渲染（受 config 门控），所以 plan 担心的「vanilla 盖掉持物」在路线 A
 * 下未必发生——须 runClient 实证。
 *
 * <p>仅 POC 用；路线拍板后本类连同 harness（{@link FpvPocControls} + playOnStack 分支 + 快捷
 * 命令）一并移除，落地形态见 plan §P1（per-animation 配置驱动 + {@code _fpv} 查找链）。
 */
public enum FpvPocState {
    /** 出厂现状：{@code THIRD_PERSON_MODEL} + 默认 config（showArm=false）——第一人称只见持物、无手臂动画。 */
    OFF,
    /** 路线 A（库原生，改动最集中）：{@code THIRD_PERSON_MODEL} + config 全开（showArm/showItem=true）。 */
    A,
    /** 路线 B（自绘，工作量最大）：{@code NONE}（库在 FP 透明，vanilla 自渲手臂）+ 自绘第一人称手臂层（渲染器 P0 待补）。 */
    B,
    /** 路线 C（vanilla 注入）：{@code VANILLA}（走 vanilla FP 手臂）+ mixin 注入动画骨骼变换（注入 P0 待补）。 */
    C;

    private static volatile FpvPocState current = OFF;

    public static FpvPocState current() {
        return current;
    }

    /** 循环到下一路线（键位触发）。返回切换后的路线。 */
    public static FpvPocState cycle() {
        FpvPocState[] all = values();
        current = all[(current.ordinal() + 1) % all.length];
        return current;
    }

    public static void set(FpvPocState next) {
        if (next != null) {
            current = next;
        }
    }

    /**
     * 把本路线的第一人称模式 + 配置应用到一个 {@link KeyframeAnimationPlayer}。
     *
     * <p>纯函数、无副作用于全局状态，便于单测直接断言 mode/config（见 {@code FpvPocStateTest}）。
     * B/C 只设基础 mode（自绘层 / 注入 mixin 属另建的渲染路径，非本方法职责）。
     */
    public void applyTo(KeyframeAnimationPlayer framePlayer) {
        switch (this) {
            case A -> framePlayer
                .setFirstPersonConfiguration(
                    new FirstPersonConfiguration()
                        .setShowRightArm(true)
                        .setShowLeftArm(true)
                        .setShowRightItem(true)
                        .setShowLeftItem(true))
                .setFirstPersonMode(FirstPersonMode.THIRD_PERSON_MODEL);
            case B -> framePlayer.setFirstPersonMode(FirstPersonMode.NONE);
            case C -> framePlayer.setFirstPersonMode(FirstPersonMode.VANILLA);
            // OFF：出厂行为——THIRD_PERSON_MODEL + 默认（arms 隐藏）config，不动 config。
            case OFF -> framePlayer.setFirstPersonMode(FirstPersonMode.THIRD_PERSON_MODEL);
        }
    }
}
