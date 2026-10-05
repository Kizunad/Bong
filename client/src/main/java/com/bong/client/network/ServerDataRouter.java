package com.bong.client.network;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/**
 * Routes the legacy JSON envelope produced by {@link ProtoServerDataBridge}.
 *
 * <p>The default table is assembled by domain registries. Those registries only
 * register existing consumers; production traffic migration remains owned by
 * the corresponding R6 merge unit.</p>
 */
public final class ServerDataRouter {
    private final Map<String, ServerDataHandler> handlers;

    public ServerDataRouter(Map<String, ServerDataHandler> handlers) {
        this.handlers = Map.copyOf(handlers);
    }

    /**
     * Builds the authoritative default type table from the domain registries.
     *
     * <p>Registry order is stable for diagnostics, while routing semantics and
     * the registered type set remain the same as the pre-RF-26 table.</p>
     */
    public static ServerDataRouter createDefault() {
        Map<String, ServerDataHandler> handlers = new LinkedHashMap<>();
        CoreServerDataRegistry.register(handlers);
        BotanyServerDataRegistry.register(handlers);
        AlchemyServerDataRegistry.register(handlers);
        CombatServerDataRegistry.register(handlers);
        WorldActivityServerDataRegistry.register(handlers);
        ForgeServerDataRegistry.register(handlers);
        SocialServerDataRegistry.register(handlers);
        CraftServerDataRegistry.register(handlers);
        SpecializedServerDataRegistry.register(handlers);
        return new ServerDataRouter(handlers);
    }

    /** Returns the immutable set of registered payload type names. */
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
