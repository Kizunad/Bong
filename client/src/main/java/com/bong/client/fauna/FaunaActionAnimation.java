package com.bong.client.fauna;

/**
 * {@link FaunaEntity} 一次性招式动画的纯状态机（无 Minecraft 依赖，可纯 JVM 单测）。
 *
 * <p>语义：服务端 emit {@code play_entity_anim} → {@link FaunaEntity#triggerAction} → 本机
 * {@link #trigger}。每 tick {@link FaunaEntity#tick()} 调一次 {@link #tick()}，倒计时归零后清空，
 * controller 据 {@link #currentAnim()} 选播招式动画 or 回 idle loop。
 *
 * <p>抽成独立纯类的原因：纯 JUnit 无法 bootstrap MC {@code Entity} / GeckoLib 运行时，把可测的
 * 倒计时逻辑下沉到这里饱和覆盖，{@link FaunaEntity} 只做薄委托。
 */
public final class FaunaActionAnimation {
    private String anim = null;
    private int ticks = 0;

    /**
     * 触发一次性招式动画，持续 {@code durationTicks} tick。
     *
     * <p>防御：{@code animName} 为 null/空白 或 {@code durationTicks <= 0} 时忽略（不改变现态），
     * 避免无效输入把正在播的动画清掉或卡死一个空动画。
     *
     * @return {@code true} 成功进入招式态（有效 payload）；{@code false} 被拒（无效输入，现态不变）。
     *     供上游把"无效 payload"如实记成未处理（bridgeMiss）而非吞掉当 handled。
     */
    public boolean trigger(String animName, int durationTicks) {
        if (animName == null || animName.isBlank() || durationTicks <= 0) {
            return false;
        }
        this.anim = animName;
        this.ticks = durationTicks;
        return true;
    }

    /** 推进一 tick；倒计时归零时清空当前招式（回到无动画态，controller 改播 idle）。 */
    public void tick() {
        if (ticks > 0) {
            ticks--;
            if (ticks <= 0) {
                anim = null;
            }
        }
    }

    /** 是否正在播一次性招式动画。 */
    public boolean hasAction() {
        return anim != null;
    }

    /** 当前招式动画名；无招式时为 null。 */
    public String currentAnim() {
        return anim;
    }

    /** 剩余 tick（用于 inspection / 测试）。 */
    public int remainingTicks() {
        return ticks;
    }
}
