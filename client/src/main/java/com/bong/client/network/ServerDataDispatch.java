package com.bong.client.network;

import java.util.Objects;
import java.util.Optional;

public final class ServerDataDispatch {
    private final String routeType;
    private final boolean handled;
    private final String logMessage;
    private final String legacyMessage;

    private ServerDataDispatch(String routeType, boolean handled, String logMessage, String legacyMessage) {
        this.routeType = Objects.requireNonNull(routeType, "routeType");
        this.handled = handled;
        this.logMessage = Objects.requireNonNull(logMessage, "logMessage");
        this.legacyMessage = legacyMessage;
    }

    public static ServerDataDispatch handled(String routeType, String logMessage) {
        return new ServerDataDispatch(routeType, true, logMessage, null);
    }

    public static ServerDataDispatch handledWithLegacyMessage(String routeType, String legacyMessage, String logMessage) {
        return new ServerDataDispatch(routeType, true, logMessage, Objects.requireNonNull(legacyMessage, "legacyMessage"));
    }

    public static ServerDataDispatch noOp(String routeType, String logMessage) {
        return new ServerDataDispatch(routeType, false, logMessage, null);
    }

    public String routeType() {
        return routeType;
    }

    public boolean handled() {
        return handled;
    }

    public String logMessage() {
        return logMessage;
    }

    public Optional<String> legacyMessage() {
        return Optional.ofNullable(legacyMessage);
    }
}
