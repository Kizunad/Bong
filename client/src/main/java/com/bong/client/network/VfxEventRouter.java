package com.bong.client.network;

import java.util.Objects;

/**
 * `bong:vfx_event` 通道路由器：解析 → 分发。与 {@link ServerDataRouter} 平级但更简单——
 * 消费侧只是调一次 API，不产生 HUD state。
 *
 * <p>两个 bridge：
 * <ul>
 *   <li>{@link VfxEventAnimationBridge}：{@code play_anim} / {@code stop_anim} → 骨骼动画</li>
 *   <li>{@link VfxParticleBridge}：{@code spawn_particle} → 粒子引擎
 *       （plan-particle-system-v1 §2.7 {@code VfxRegistry} 查表）</li>
 * </ul>
 *
 * <p>失败处理按三档：
 * <ol>
 *   <li>解析失败 → {@link RouteResult#parseError}，调用方打 error 日志</li>
 *   <li>bridge 返回 false（玩家不在线 / 事件 id 未注册） → {@link RouteResult#bridgeMiss}，
 *       调用方按节流策略降级 warn</li>
 *   <li>成功 → {@link RouteResult#handled}，调用方打 info</li>
 * </ol>
 *
 * <p>bridge 抛异常时路由器把它转成 bridgeMiss 以避免客户端循环崩溃——服务端已经发包了，
 * 一个格式正确但运行期失败的事件不应撕裂整个网络层。
 */
public final class VfxEventRouter {
    private final VfxEventAnimationBridge animationBridge;
    private final VfxParticleBridge particleBridge;

    public VfxEventRouter(VfxEventAnimationBridge animationBridge) {
        this(animationBridge, VfxParticleBridge.noop());
    }

    public VfxEventRouter(
        VfxEventAnimationBridge animationBridge,
        VfxParticleBridge particleBridge
    ) {
        this.animationBridge = Objects.requireNonNull(animationBridge, "animationBridge");
        this.particleBridge = Objects.requireNonNull(particleBridge, "particleBridge");
    }

    public RouteResult route(String jsonPayload, int payloadSizeBytes) {
        VfxEventParseResult parseResult = VfxEventEnvelope.parse(jsonPayload, payloadSizeBytes);
        if (!parseResult.isSuccess()) {
            return RouteResult.parseError(parseResult.errorMessage());
        }
        return route(parseResult.payload());
    }

    public RouteResult route(VfxEventPayload payload) {
        Objects.requireNonNull(payload, "payload");
        try {
            boolean ok;
            String missContext;
            if (payload instanceof VfxEventPayload.PlayAnim play) {
                ok = animationBridge.playAnim(
                    play.targetPlayer(),
                    play.animId(),
                    play.priority(),
                    play.fadeInTicks()
                );
                missContext = "bridge declined play_anim " + play.animId() + " on " + play.targetPlayer();
            } else if (payload instanceof VfxEventPayload.PlayAnimInline inline) {
                ok = animationBridge.playAnimInline(
                    inline.targetPlayer(),
                    inline.animId(),
                    inline.animJson(),
                    inline.priority(),
                    inline.fadeInTicks()
                );
                missContext = "bridge declined play_anim_inline " + inline.animId() + " on " + inline.targetPlayer();
            } else if (payload instanceof VfxEventPayload.StopAnim stop) {
                ok = animationBridge.stopAnim(
                    stop.targetPlayer(),
                    stop.animId(),
                    stop.fadeOutTicks()
                );
                missContext = "bridge declined stop_anim " + stop.animId() + " on " + stop.targetPlayer();
            } else if (payload instanceof VfxEventPayload.SpawnParticle particle) {
                ok = particleBridge.spawnParticle(particle);
                missContext = "bridge declined spawn_particle " + particle.eventId();
            } else {
                throw new IllegalStateException("Unhandled VfxEventPayload variant: " + payload.getClass().getName());
            }
            if (ok) {
                return RouteResult.handled(payload);
            }
            return RouteResult.bridgeMiss(payload, missContext);
        } catch (RuntimeException exception) {
            return RouteResult.bridgeMiss(
                payload,
                "bridge threw " + exception.getClass().getSimpleName() + ": " + exception.getMessage()
            );
        }
    }

    /** 解析 + 分发结果的联合类型。 */
    public static final class RouteResult {
        private final Kind kind;
        private final VfxEventPayload payload;
        private final String logMessage;

        private RouteResult(Kind kind, VfxEventPayload payload, String logMessage) {
            this.kind = kind;
            this.payload = payload;
            this.logMessage = logMessage;
        }

        static RouteResult parseError(String logMessage) {
            return new RouteResult(Kind.PARSE_ERROR, null, logMessage);
        }

        static RouteResult handled(VfxEventPayload payload) {
            return new RouteResult(
                Kind.HANDLED,
                payload,
                "dispatched " + payload.debugDescriptor()
            );
        }

        static RouteResult bridgeMiss(VfxEventPayload payload, String reason) {
            return new RouteResult(Kind.BRIDGE_MISS, payload, reason);
        }

        public Kind kind() {
            return kind;
        }

        public VfxEventPayload payload() {
            return payload;
        }

        public String logMessage() {
            return logMessage;
        }

        public boolean isParseError() {
            return kind == Kind.PARSE_ERROR;
        }

        public boolean isHandled() {
            return kind == Kind.HANDLED;
        }

        public boolean isBridgeMiss() {
            return kind == Kind.BRIDGE_MISS;
        }

        public enum Kind {
            PARSE_ERROR,
            HANDLED,
            BRIDGE_MISS,
        }
    }
}
