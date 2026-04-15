package com.bong.client.network;

import net.minecraft.util.Identifier;

import java.util.Objects;
import java.util.OptionalInt;
import java.util.UUID;

/**
 * `bong:vfx_event` CustomPayload 解析后的强类型载荷。
 *
 * <p>与 {@code agent/packages/schema/src/vfx-event.ts} / {@code server/src/schema/vfx_event.rs}
 * 一一对应；当前 Phase 1 仅两个 variant，未来扩 particle/sword_qi_slash 等粒子事件时在此
 * {@code permits} 列表里新增。
 *
 * <p>为什么要 sealed interface：
 * <ul>
 *   <li>消费侧（{@link VfxEventRouter}）用 {@code switch} 模式匹配穷举，编译器能守住新 variant
 *       漏处理的错误</li>
 *   <li>解析层把 JSON 字段语义抬成 Java 类型（UUID、Identifier、OptionalInt），路由层免得重复
 *       做字段验证</li>
 * </ul>
 */
public sealed interface VfxEventPayload
    permits VfxEventPayload.PlayAnim, VfxEventPayload.StopAnim {

    /** JSON 里的 `type` 字段原值。主要给日志用。 */
    String type();

    /** 目标玩家 UUID。若当前世界不在线，路由层选择 no-op。 */
    UUID targetPlayer();

    /** 动画 id（MC Identifier）。注册表里不存在时 {@link com.bong.client.animation.BongAnimationPlayer#play} 会返回 false。 */
    Identifier animId();

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
    }
}
