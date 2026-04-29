package com.bong.client.network;

import net.minecraft.util.Identifier;

import java.util.Objects;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.UUID;

/**
 * `bong:vfx_event` CustomPayload 解析后的强类型载荷。
 *
 * <p>与 {@code agent/packages/schema/src/vfx-event.ts} / {@code server/src/schema/vfx_event.rs}
 * 一一对应。当前四个 variant：
 * <ul>
 *   <li>{@link PlayAnim} / {@link StopAnim}：玩家骨骼动画触发（plan-player-animation-v1）</li>
 *   <li>{@link PlayAnimInline}：运行时注入完整 PlayerAnimator JSON 并立即播放</li>
 *   <li>{@link SpawnParticle}：世界内粒子触发（plan-particle-system-v1 §2.2）</li>
 * </ul>
 *
 * <p>为什么是 sealed interface：消费侧（{@link VfxEventRouter}）用 {@code instanceof} / switch
 * 模式匹配穷举，编译器能守住新 variant 漏处理的错误。
 *
 * <p>注意：早期版本在基接口上暴露 {@code targetPlayer()} / {@code animId()}，
 * 加入粒子 variant 后这两个字段不通用（粒子没有目标玩家），故仅保留 {@link #type()} 和
 * {@link #debugDescriptor()}，具体字段由模式匹配取。
 */
public sealed interface VfxEventPayload
    permits VfxEventPayload.PlayAnim, VfxEventPayload.PlayAnimInline,
    VfxEventPayload.StopAnim, VfxEventPayload.SpawnParticle {

    /** JSON 里的 `type` 字段原值，仅给日志用。 */
    String type();

    /**
     * 供日志聚合用的简短描述（`<type> id=<anim_id|event_id> target=<uuid|-->`）。
     * 不保证稳定格式，仅用于 debug。
     */
    String debugDescriptor();

    record PlayAnim(
        UUID targetPlayer,
        Identifier animId,
        int priority,
        OptionalInt fadeInTicks
    ) implements VfxEventPayload {
        public PlayAnim {
            Objects.requireNonNull(targetPlayer, "targetPlayer");
            Objects.requireNonNull(animId, "animId");
            Objects.requireNonNull(fadeInTicks, "fadeInTicks");
        }

        @Override
        public String type() {
            return "play_anim";
        }

        @Override
        public String debugDescriptor() {
            return "play_anim anim=" + animId + " target=" + targetPlayer;
        }
    }

    record PlayAnimInline(
        UUID targetPlayer,
        Identifier animId,
        String animJson,
        int priority,
        OptionalInt fadeInTicks
    ) implements VfxEventPayload {
        public PlayAnimInline {
            Objects.requireNonNull(targetPlayer, "targetPlayer");
            Objects.requireNonNull(animId, "animId");
            Objects.requireNonNull(animJson, "animJson");
            Objects.requireNonNull(fadeInTicks, "fadeInTicks");
        }

        @Override
        public String type() {
            return "play_anim_inline";
        }

        @Override
        public String debugDescriptor() {
            return "play_anim_inline anim=" + animId + " target=" + targetPlayer;
        }
    }

    record StopAnim(
        UUID targetPlayer,
        Identifier animId,
        OptionalInt fadeOutTicks
    ) implements VfxEventPayload {
        public StopAnim {
            Objects.requireNonNull(targetPlayer, "targetPlayer");
            Objects.requireNonNull(animId, "animId");
            Objects.requireNonNull(fadeOutTicks, "fadeOutTicks");
        }

        @Override
        public String type() {
            return "stop_anim";
        }

        @Override
        public String debugDescriptor() {
            return "stop_anim anim=" + animId + " target=" + targetPlayer;
        }
    }

    /**
     * 粒子触发（plan-particle-system-v1 §2.2）。
     *
     * <p>解析后的 {@code eventId} 是已校验 MC {@link Identifier}。可选字段使用
     * {@link Optional} 以便在渲染侧用 {@code ifPresent} 组合；数值字段用 {@link Optional}
     * 而非原始型（如 {@code OptionalInt}）是因为 {@code strength} 是 double，
     * 保持一致外观比 mix {@link java.util.OptionalDouble} 更可读。
     *
     * @param eventId        粒子事件 id，客户端按此查 {@code VfxRegistry}
     * @param origin         世界坐标原点（正交 Vec3，已校验 finite）
     * @param direction      可选方向向量（未归一，客户端按需处理）
     * @param colorRgb       可选 0xRRGGBB 颜色（JSON `#RRGGBB` 解析结果）
     * @param strength       可选归一化强度 [0, 1]
     * @param count          可选合批数量 [1, {@value VfxEventEnvelope#VFX_PARTICLE_COUNT_MAX}]
     * @param durationTicks  可选持续 tick 数 [1, {@value VfxEventEnvelope#VFX_PARTICLE_DURATION_TICKS_MAX}]
     */
    record SpawnParticle(
        Identifier eventId,
        double[] origin,
        Optional<double[]> direction,
        OptionalInt colorRgb,
        Optional<Double> strength,
        OptionalInt count,
        OptionalInt durationTicks
    ) implements VfxEventPayload {
        public SpawnParticle {
            Objects.requireNonNull(eventId, "eventId");
            Objects.requireNonNull(origin, "origin");
            if (origin.length != 3) {
                throw new IllegalArgumentException("origin must be length 3, got " + origin.length);
            }
            Objects.requireNonNull(direction, "direction");
            direction.ifPresent(d -> {
                if (d.length != 3) {
                    throw new IllegalArgumentException(
                        "direction must be length 3, got " + d.length);
                }
            });
            Objects.requireNonNull(colorRgb, "colorRgb");
            Objects.requireNonNull(strength, "strength");
            Objects.requireNonNull(count, "count");
            Objects.requireNonNull(durationTicks, "durationTicks");
        }

        @Override
        public String type() {
            return "spawn_particle";
        }

        @Override
        public String debugDescriptor() {
            return "spawn_particle event=" + eventId + " origin=["
                + origin[0] + "," + origin[1] + "," + origin[2] + "]";
        }
    }
}
