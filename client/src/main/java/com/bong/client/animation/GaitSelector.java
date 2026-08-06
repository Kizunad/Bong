package com.bong.client.animation;

import com.bong.client.movement.MovementState;
import net.minecraft.util.Identifier;

/**
 * 下半身步态档位选择——纯函数，不碰 MC 运行时，便于饱和单测。
 *
 * <p>档位来源（2026-08-06 决定：复用 vanilla 状态 + 速度倍率，不动 server/schema）：
 * <ul>
 *   <li>{@link Gait#DASH} —— {@link MovementState.Action#DASHING}，服务端权威动作</li>
 *   <li>{@link Gait#SPRINT} —— {@code currentSpeedMultiplier} 超过 {@link #SPRINT_SPEED_THRESHOLD}
 *       （身法/灵气加速档）</li>
 *   <li>{@link Gait#JOG} —— vanilla {@code player.isSprinting()}（Ctrl 疾跑）</li>
 *   <li>{@link Gait#WALK} —— 有水平位移</li>
 *   <li>{@link Gait#NONE} —— 静止或离地（跳跃/坠落不该播步态；DASH 例外，它本就腾空）</li>
 * </ul>
 *
 * <p>优先级自上而下，先匹配先返回。
 */
public final class GaitSelector {
    /** 速度倍率超过此值算"冲刺"档。基础倍率 1.0，vanilla sprint 不改这个值。 */
    public static final double SPRINT_SPEED_THRESHOLD = 1.35;
    /** 水平速度低于此值（格/tick）算静止——手柄/网络抖动不该触发走路。 */
    public static final double MOTION_EPSILON = 0.02;

    private GaitSelector() {
    }

    public enum Gait {
        NONE(null, false),
        WALK("lower_walk", true),
        JOG("lower_jog", true),
        SPRINT("lower_sprint", true),
        DASH("lower_dash", false, 0, 0);

        private final Identifier animId;
        private final boolean looped;
        private final int fadeInTicks;
        private final int fadeOutTicks;

        Gait(String path, boolean looped) {
            this(path, looped, BongAnimationPlayer.DEFAULT_FADE_IN_TICKS,
                BongAnimationPlayer.DEFAULT_FADE_OUT_TICKS);
        }

        Gait(String path, boolean looped, int fadeInTicks, int fadeOutTicks) {
            this.animId = path == null ? null : new Identifier("bong", path);
            this.looped = looped;
            this.fadeInTicks = fadeInTicks;
            this.fadeOutTicks = fadeOutTicks;
        }

        /** 对应的下半身动画 id；{@link #NONE} 返回 null（= 停掉本通道）。 */
        public Identifier animId() {
            return animId;
        }

        /** 循环步态 vs 一次性（dash）。 */
        public boolean looped() {
            return looped;
        }

        public int fadeInTicks() {
            return fadeInTicks;
        }

        public int fadeOutTicks() {
            return fadeOutTicks;
        }
    }

    /**
     * @param dashing         服务端下发的 DASHING 动作
     * @param speedMultiplier 服务端下发的当前速度倍率
     * @param sprinting       vanilla {@code player.isSprinting()}
     * @param horizontalSpeed 水平速度（格/tick）
     * @param onGround        是否着地
     */
    public record GaitInput(
        boolean dashing,
        double speedMultiplier,
        boolean sprinting,
        double horizontalSpeed,
        boolean onGround
    ) {
    }

    public static Gait select(GaitInput in) {
        if (in == null) {
            return Gait.NONE;
        }
        if (in.dashing()) {
            return Gait.DASH;
        }
        if (!in.onGround()) {
            return Gait.NONE;
        }
        if (in.horizontalSpeed() < MOTION_EPSILON) {
            return Gait.NONE;
        }
        if (in.speedMultiplier() > SPRINT_SPEED_THRESHOLD) {
            return Gait.SPRINT;
        }
        if (in.sprinting()) {
            return Gait.JOG;
        }
        return Gait.WALK;
    }
}
