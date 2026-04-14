package com.bong.client.network;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

public final class ServerDataRouter {
    private final Map<String, ServerDataHandler> handlers;

    public ServerDataRouter(Map<String, ServerDataHandler> handlers) {
        this.handlers = Map.copyOf(handlers);
    }

    public static ServerDataRouter createDefault() {
        LegacyMessageServerDataHandler legacyHandler = new LegacyMessageServerDataHandler();
        NarrationHandler narrationHandler = new NarrationHandler();
        ZoneInfoHandler zoneInfoHandler = new ZoneInfoHandler();
        EventAlertHandler eventAlertHandler = new EventAlertHandler();
        PlayerStateHandler playerStateHandler = new PlayerStateHandler();
        UiOpenHandler uiOpenHandler = new UiOpenHandler();
        CultivationDetailHandler cultivationDetailHandler = new CultivationDetailHandler();
        InventorySnapshotHandler inventorySnapshotHandler = new InventorySnapshotHandler();
        InventoryEventHandler inventoryEventHandler = new InventoryEventHandler();

        Map<String, ServerDataHandler> handlers = new LinkedHashMap<>();
        handlers.put("welcome", legacyHandler);
        handlers.put("heartbeat", legacyHandler);
        handlers.put("narration", narrationHandler);
        handlers.put("zone_info", zoneInfoHandler);
        handlers.put("event_alert", eventAlertHandler);
        handlers.put("player_state", playerStateHandler);
        handlers.put("ui_open", uiOpenHandler);
        handlers.put("cultivation_detail", cultivationDetailHandler);
        handlers.put("inventory_snapshot", inventorySnapshotHandler);
        handlers.put("inventory_event", inventoryEventHandler);
        return new ServerDataRouter(handlers);
    }

    public Set<String> registeredTypes() {
        return handlers.keySet();
    }

    public RouteResult route(String jsonPayload, int payloadSizeBytes) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(jsonPayload, payloadSizeBytes);
        if (!parseResult.isSuccess()) {
            return RouteResult.parseError(parseResult);
        }

        return route(parseResult.envelope());
    }

    public RouteResult route(ServerDataEnvelope envelope) {
        Objects.requireNonNull(envelope, "envelope");

        ServerDataHandler handler = handlers.get(envelope.type());
        if (handler == null) {
            return RouteResult.dispatched(
                ServerPayloadParseResult.success(envelope),
                ServerDataDispatch.noOp(
                    envelope.type(),
                    "No registered handler for payload type '" + envelope.type() + "'; payload ignored safely"
                )
            );
        }

        try {
            return RouteResult.dispatched(ServerPayloadParseResult.success(envelope), handler.handle(envelope));
        } catch (RuntimeException exception) {
            return RouteResult.dispatched(
                ServerPayloadParseResult.success(envelope),
                ServerDataDispatch.noOp(
                    envelope.type(),
                    "Handler for payload type '" + envelope.type() + "' failed safely: " + exception.getMessage()
                )
            );
        }
    }

    public static final class RouteResult {
        private final ServerPayloadParseResult parseResult;
        private final ServerDataDispatch dispatch;

        private RouteResult(ServerPayloadParseResult parseResult, ServerDataDispatch dispatch) {
            this.parseResult = parseResult;
            this.dispatch = dispatch;
        }

        private static RouteResult parseError(ServerPayloadParseResult parseResult) {
            return new RouteResult(parseResult, null);
        }

        private static RouteResult dispatched(ServerPayloadParseResult parseResult, ServerDataDispatch dispatch) {
            return new RouteResult(parseResult, dispatch);
        }

        public ServerPayloadParseResult parseResult() {
            return parseResult;
        }

        public ServerDataEnvelope envelope() {
            return parseResult.envelope();
        }

        public ServerDataDispatch dispatch() {
            return dispatch;
        }

        public boolean isParseError() {
            return !parseResult.isSuccess();
        }

        public boolean isHandled() {
            return dispatch != null && dispatch.handled();
        }

        public boolean isNoOp() {
            return dispatch != null && !dispatch.handled();
        }

        public String logMessage() {
            if (dispatch != null) {
                return dispatch.logMessage();
            }
            return parseResult.errorMessage();
        }
    }
}
