package com.bong.client.network;

import java.util.Objects;

/**
 * `bong:vfx_event` 通道路由器：解析 → 分发。与 {@link ServerDataRouter} 平级但更简单——
 * 消费侧只是调一次 API，不产生 HUD state。
 *
 * <p>失败处理按三档：
 * <ol>
 *   <li>解析失败 → {@link RouteResult#parseError}，调用方打 error 日志</li>
 *   <li>bridge 返回 false（玩家不在线 / 动画未注册） → {@link RouteResult#bridgeMiss}，
 *       调用方按节流策略降级 warn</li>
 *   <li>成功 → {@link RouteResult#handled}，调用方打 info</li>
 * </ol>
 *
 * <p>bridge 抛异常时路由器把它转成 bridgeMiss 以避免客户端循环崩溃——服务端已经发包了，
 * 一个格式正确但运行期失败的事件不应撕裂整个网络层。
 */
public final class VfxEventRouter {
    private final VfxEventAnimationBridge bridge;

    public VfxEventRouter(VfxEventAnimationBridge bridge) {
        this.bridge = Objects.requireNonNull(bridge, "bridge");
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
            // Java 17：instanceof pattern 是标准特性，switch pattern 还没出，
            // 所以维持 if-else 形式。新 variant 加进来时编译器会提示 permits 穷举
            // 类型检查，依然安全。
            boolean ok;
            if (payload instanceof VfxEventPayload.PlayAnim play) {
                ok = bridge.playAnim(
                    play.targetPlayer(),
                    play.animId(),
                    play.priority(),
                    play.fadeInTicks()
                );
            } else if (payload instanceof VfxEventPayload.StopAnim stop) {
                ok = bridge.stopAnim(
                    stop.targetPlayer(),
                    stop.animId(),
                    stop.fadeOutTicks()
                );
            } else {
                throw new IllegalStateException("Unhandled VfxEventPayload variant: " + payload.getClass().getName());
            }
            if (ok) {
                return RouteResult.handled(payload);
            }
            return RouteResult.bridgeMiss(
                payload,
                "bridge declined " + payload.type() + " for " + payload.animId() + " on " + payload.targetPlayer()
            );
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
                "dispatched " + payload.type() + " anim=" + payload.animId() + " target=" + payload.targetPlayer()
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
