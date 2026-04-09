package com.bong.client;

import java.util.List;
import java.util.Objects;

public sealed interface BongServerPayload permits BongServerPayload.WelcomePayload,
        BongServerPayload.HeartbeatPayload,
        BongServerPayload.NarrationPayload,
        BongServerPayload.ZoneInfoPayload,
        BongServerPayload.EventAlertPayload,
        BongServerPayload.PlayerStatePayload {

    int v();

    BongServerPayloadKind kind();

    default int version() {
        return v();
    }

    default String type() {
        return kind().wireName();
    }

    enum BongServerPayloadKind {
        WELCOME("welcome"),
        HEARTBEAT("heartbeat"),
        NARRATION("narration"),
        ZONE_INFO("zone_info"),
        EVENT_ALERT("event_alert"),
        PLAYER_STATE("player_state");

        private final String wireName;

        BongServerPayloadKind(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }

        public static BongServerPayloadKind fromWireName(String wireName) {
            for (BongServerPayloadKind kind : values()) {
                if (kind.wireName.equals(wireName)) {
                    return kind;
                }
            }

            return null;
        }
    }

    record WelcomePayload(int v, String message) implements BongServerPayload {
        public WelcomePayload {
            Objects.requireNonNull(message, "message");
        }

        @Override
        public BongServerPayloadKind kind() {
            return BongServerPayloadKind.WELCOME;
        }
    }

    record HeartbeatPayload(int v, String message) implements BongServerPayload {
        public HeartbeatPayload {
            Objects.requireNonNull(message, "message");
        }

        @Override
        public BongServerPayloadKind kind() {
            return BongServerPayloadKind.HEARTBEAT;
        }
    }

    record Narration(String scope, String text, String style) {
        public Narration {
            Objects.requireNonNull(scope, "scope");
            Objects.requireNonNull(text, "text");
            Objects.requireNonNull(style, "style");
        }
    }

    record NarrationPayload(int v, List<Narration> narrations) implements BongServerPayload {
        public NarrationPayload {
            narrations = List.copyOf(Objects.requireNonNull(narrations, "narrations"));
        }

        @Override
        public BongServerPayloadKind kind() {
            return BongServerPayloadKind.NARRATION;
        }
    }

    record ZoneInfo(String zone, double spiritQi, int dangerLevel, List<String> activeEvents) {
        public ZoneInfo {
            Objects.requireNonNull(zone, "zone");
            activeEvents = activeEvents == null ? List.of() : List.copyOf(activeEvents);
        }
    }

    record ZoneInfoPayload(int v, ZoneInfo zoneInfo) implements BongServerPayload {
        public ZoneInfoPayload {
            Objects.requireNonNull(zoneInfo, "zoneInfo");
        }

        @Override
        public BongServerPayloadKind kind() {
            return BongServerPayloadKind.ZONE_INFO;
        }
    }

    record EventAlert(String kind, String title, String detail, String severity, String zone) {
        public EventAlert {
            Objects.requireNonNull(kind, "kind");
            Objects.requireNonNull(title, "title");
            Objects.requireNonNull(detail, "detail");
            Objects.requireNonNull(severity, "severity");
        }
    }

    record EventAlertPayload(int v, EventAlert eventAlert) implements BongServerPayload {
        public EventAlertPayload {
            Objects.requireNonNull(eventAlert, "eventAlert");
        }

        @Override
        public BongServerPayloadKind kind() {
            return BongServerPayloadKind.EVENT_ALERT;
        }
    }

    record PlayerState(String realm, double spiritQi, double spiritQiMax, double karma, double compositePower,
                       String zone) {
        public PlayerState {
            Objects.requireNonNull(realm, "realm");
            Objects.requireNonNull(zone, "zone");
        }
    }

    record PlayerStatePayload(int v, PlayerState playerState) implements BongServerPayload {
        public PlayerStatePayload {
            Objects.requireNonNull(playerState, "playerState");
        }

        @Override
        public BongServerPayloadKind kind() {
            return BongServerPayloadKind.PLAYER_STATE;
        }
    }
}
