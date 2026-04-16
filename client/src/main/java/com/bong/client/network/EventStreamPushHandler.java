package com.bong.client.network;

import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * plan-HUD-v1 §2.3 / §11.4 event_stream_push 客户端 handler。
 * server 把 CombatEvent / DeathEvent 等翻译后逐条推送，本 handler 写入
 * {@link UnifiedEventStore} 的 stream，{@code EventStreamHudPlanner} 滚动渲染。
 */
public final class EventStreamPushHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String channelStr = readString(payload, "channel");
        String priorityStr = readString(payload, "priority");
        String sourceTag = readString(payload, "source_tag");
        String text = readString(payload, "text");
        Long color = readLong(payload, "color");
        Long createdAtMs = readLong(payload, "created_at_ms");

        if (channelStr == null || priorityStr == null || text == null
            || color == null || createdAtMs == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring event_stream_push payload: required fields missing or invalid");
        }

        UnifiedEvent.Channel channel = parseChannel(channelStr);
        UnifiedEvent.Priority priority = parsePriority(priorityStr);
        if (channel == null || priority == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring event_stream_push payload: unknown channel '" + channelStr
                    + "' or priority '" + priorityStr + "'");
        }

        int effectiveColor = color.intValue() == 0 ? channel.defaultColor() : color.intValue();
        boolean accepted = UnifiedEventStore.stream().publish(
            channel,
            priority,
            sourceTag == null ? "" : sourceTag,
            text,
            effectiveColor,
            createdAtMs
        );

        return ServerDataDispatch.handled(envelope.type(),
            "event_stream_push " + (accepted ? "accepted" : "throttled")
                + " (channel=" + channelStr + " text=\"" + text + "\")");
    }

    private static UnifiedEvent.Channel parseChannel(String wire) {
        return switch (wire) {
            case "combat" -> UnifiedEvent.Channel.COMBAT;
            case "cultivation" -> UnifiedEvent.Channel.CULTIVATION;
            case "world" -> UnifiedEvent.Channel.WORLD;
            case "social" -> UnifiedEvent.Channel.SOCIAL;
            case "system" -> UnifiedEvent.Channel.SYSTEM;
            default -> null;
        };
    }

    private static UnifiedEvent.Priority parsePriority(String wire) {
        return switch (wire) {
            case "p0_critical" -> UnifiedEvent.Priority.P0_CRITICAL;
            case "p1_important" -> UnifiedEvent.Priority.P1_IMPORTANT;
            case "p2_normal" -> UnifiedEvent.Priority.P2_NORMAL;
            case "p3_verbose" -> UnifiedEvent.Priority.P3_VERBOSE;
            default -> null;
        };
    }

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : null;
    }

    private static Long readLong(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isNumber() ? p.getAsLong() : null;
    }
}
