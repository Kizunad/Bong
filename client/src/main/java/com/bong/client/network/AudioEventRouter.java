package com.bong.client.network;

import java.util.Objects;

public final class AudioEventRouter {
    private final AudioPlaybackBridge bridge;

    public AudioEventRouter(AudioPlaybackBridge bridge) {
        this.bridge = Objects.requireNonNull(bridge, "bridge");
    }

    public RouteResult routePlay(String jsonPayload, int payloadSizeBytes) {
        AudioEventParseResult parseResult = AudioEventEnvelope.parsePlay(jsonPayload, payloadSizeBytes);
        if (!parseResult.isSuccess()) {
            return RouteResult.parseError(parseResult.errorMessage());
        }
        return route(parseResult.payload());
    }

    public RouteResult routeStop(String jsonPayload, int payloadSizeBytes) {
        AudioEventParseResult parseResult = AudioEventEnvelope.parseStop(jsonPayload, payloadSizeBytes);
        if (!parseResult.isSuccess()) {
            return RouteResult.parseError(parseResult.errorMessage());
        }
        return route(parseResult.payload());
    }

    public RouteResult route(AudioEventPayload payload) {
        Objects.requireNonNull(payload, "payload");
        try {
            boolean ok;
            String missContext;
            if (payload instanceof AudioEventPayload.PlaySoundRecipe play) {
                ok = bridge.play(play);
                missContext = "bridge declined play recipe " + play.recipeId() + "#" + play.instanceId();
            } else if (payload instanceof AudioEventPayload.StopSoundRecipe stop) {
                ok = bridge.stop(stop);
                missContext = "bridge declined stop instance #" + stop.instanceId();
            } else {
                throw new IllegalStateException("Unhandled AudioEventPayload variant: " + payload.getClass().getName());
            }
            return ok ? RouteResult.handled(payload) : RouteResult.bridgeMiss(payload, missContext);
        } catch (RuntimeException exception) {
            return RouteResult.bridgeMiss(
                payload,
                "bridge threw " + exception.getClass().getSimpleName() + ": " + exception.getMessage()
            );
        }
    }

    public static final class RouteResult {
        private final Kind kind;
        private final AudioEventPayload payload;
        private final String logMessage;

        private RouteResult(Kind kind, AudioEventPayload payload, String logMessage) {
            this.kind = kind;
            this.payload = payload;
            this.logMessage = logMessage;
        }

        static RouteResult parseError(String logMessage) {
            return new RouteResult(Kind.PARSE_ERROR, null, logMessage);
        }

        static RouteResult handled(AudioEventPayload payload) {
            return new RouteResult(Kind.HANDLED, payload, "dispatched " + payload.debugDescriptor());
        }

        static RouteResult bridgeMiss(AudioEventPayload payload, String reason) {
            return new RouteResult(Kind.BRIDGE_MISS, payload, reason);
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

        public AudioEventPayload payload() {
            return payload;
        }

        public String logMessage() {
            return logMessage;
        }

        public enum Kind {
            PARSE_ERROR,
            HANDLED,
            BRIDGE_MISS,
        }
    }
}
