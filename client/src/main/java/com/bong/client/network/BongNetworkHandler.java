package com.bong.client.network;

import com.bong.client.BongClient;
import com.bong.client.network.handlers.*;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.util.Identifier;

import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;

public class BongNetworkHandler {
    public static final int EXPECTED_VERSION = 1;

    private static final Map<String, PayloadHandler> handlers = new HashMap<>();

    static {
        handlers.put("welcome", new WelcomeHandler());
        handlers.put("heartbeat", new HeartbeatHandler());
        handlers.put("narration", new NarrationHandler());
        handlers.put("zone_info", new ZoneInfoHandler());
        handlers.put("event_alert", new EventAlertHandler());
        handlers.put("player_state", new PlayerStateHandler());
        handlers.put("ui_open", new UiOpenHandler());
    }

    public static void register() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "server_data"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = new String(bytes, StandardCharsets.UTF_8);
            ParseResult result = parseServerPayload(jsonPayload);

            if (result.success) {
                client.execute(() -> {
                    PayloadHandler payloadHandler = handlers.get(result.payload.type);
                    if (payloadHandler != null) {
                        payloadHandler.handle(client, result.payload.type, jsonPayload);
                    } else {
                        BongClient.LOGGER.warn("Unhandled bong:server_data payload type: {}", result.payload.type);
                    }
                });
            } else {
                BongClient.LOGGER.error("Failed to parse bong:server_data payload: {}", result.errorMessage);
            }
        });
    }

    public static ParseResult parseServerPayload(String jsonPayload) {
        try {
            ParsedPayloadFields fields = parseFields(jsonPayload);

            if (fields.version == null) {
                return ParseResult.error("Missing version 'v' field");
            }
            if (fields.version != EXPECTED_VERSION) {
                return ParseResult.error("Unsupported version: " + fields.version);
            }
            if (fields.type == null) {
                return ParseResult.error("Missing required field 'type'");
            }

            return ParseResult.success(new Payload(fields.version, fields.type));
        } catch (PayloadParseException exception) {
            return ParseResult.error(exception.getMessage());
        }
    }

    private static ParsedPayloadFields parseFields(String jsonPayload) throws PayloadParseException {
        JsonCursor cursor = new JsonCursor(jsonPayload);
        ParsedPayloadFields fields = new ParsedPayloadFields();

        cursor.skipWhitespace();
        cursor.expect('{');
        cursor.skipWhitespace();

        if (cursor.tryConsume('}')) {
            return fields;
        }

        while (true) {
            String key = cursor.readQuotedString();
            cursor.skipWhitespace();
            cursor.expect(':');
            cursor.skipWhitespace();

            switch (key) {
                case "v" -> fields.version = cursor.readInteger();
                case "type" -> fields.type = cursor.readQuotedString();
                default -> cursor.skipSimpleValue();
            }

            cursor.skipWhitespace();
            if (cursor.tryConsume(',')) {
                cursor.skipWhitespace();
                continue;
            }

            cursor.expect('}');
            cursor.skipWhitespace();
            if (!cursor.isAtEnd()) {
                throw new PayloadParseException("Malformed JSON: unexpected trailing characters");
            }
            return fields;
        }
    }

    private static final class ParsedPayloadFields {
        private Integer version;
        private String type;
    }

    private static final class JsonCursor {
        private final String input;
        private int index;

        private JsonCursor(String input) {
            this.input = input;
        }

        private void skipWhitespace() {
            while (index < input.length() && Character.isWhitespace(input.charAt(index))) {
                index++;
            }
        }

        private boolean tryConsume(char expected) {
            if (index < input.length() && input.charAt(index) == expected) {
                index++;
                return true;
            }
            return false;
        }

        private void expect(char expected) throws PayloadParseException {
            if (!tryConsume(expected)) {
                throw new PayloadParseException("Malformed JSON: expected '" + expected + "'");
            }
        }

        private String readQuotedString() throws PayloadParseException {
            expect('"');
            StringBuilder builder = new StringBuilder();

            while (index < input.length()) {
                char current = input.charAt(index++);
                if (current == '"') {
                    return builder.toString();
                }
                if (current == '\\') {
                    if (index >= input.length()) {
                        throw new PayloadParseException("Malformed JSON: invalid escape sequence");
                    }

                    char escaped = input.charAt(index++);
                    switch (escaped) {
                        case '"', '\\', '/' -> builder.append(escaped);
                        case 'b' -> builder.append('\b');
                        case 'f' -> builder.append('\f');
                        case 'n' -> builder.append('\n');
                        case 'r' -> builder.append('\r');
                        case 't' -> builder.append('\t');
                        default -> throw new PayloadParseException("Malformed JSON: unsupported escape sequence");
                    }
                    continue;
                }

                builder.append(current);
            }

            throw new PayloadParseException("Malformed JSON: unterminated string");
        }

        private int readInteger() throws PayloadParseException {
            int start = index;
            if (index < input.length() && input.charAt(index) == '-') {
                index++;
            }

            while (index < input.length() && Character.isDigit(input.charAt(index))) {
                index++;
            }

            if (start == index || (input.charAt(start) == '-' && start + 1 == index)) {
                throw new PayloadParseException("Malformed JSON: invalid integer value");
            }

            try {
                return Integer.parseInt(input.substring(start, index));
            } catch (NumberFormatException exception) {
                throw new PayloadParseException("Malformed JSON: invalid integer value");
            }
        }

        private void skipSimpleValue() throws PayloadParseException {
            if (index >= input.length()) {
                throw new PayloadParseException("Malformed JSON: missing value");
            }

            char current = input.charAt(index);
            switch (current) {
                case '"' -> readQuotedString();
                case '{' -> skipObject();
                case '[' -> skipArray();
                default -> skipLiteral();
            }
        }

        private void skipObject() throws PayloadParseException {
            expect('{');
            skipWhitespace();

            if (tryConsume('}')) {
                return;
            }

            while (true) {
                readQuotedString();
                skipWhitespace();
                expect(':');
                skipWhitespace();
                skipSimpleValue();
                skipWhitespace();

                if (tryConsume(',')) {
                    skipWhitespace();
                    continue;
                }

                expect('}');
                return;
            }
        }

        private void skipArray() throws PayloadParseException {
            expect('[');
            skipWhitespace();

            if (tryConsume(']')) {
                return;
            }

            while (true) {
                skipSimpleValue();
                skipWhitespace();

                if (tryConsume(',')) {
                    skipWhitespace();
                    continue;
                }

                expect(']');
                return;
            }
        }

        private void skipLiteral() throws PayloadParseException {
            int start = index;
            while (index < input.length()) {
                char current = input.charAt(index);
                if (current == ',' || current == '}' || current == ']' || Character.isWhitespace(current)) {
                    break;
                }
                index++;
            }

            if (start == index) {
                throw new PayloadParseException("Malformed JSON: missing value");
            }
        }

        private boolean isAtEnd() {
            return index >= input.length();
        }
    }

    private static final class PayloadParseException extends Exception {
        private PayloadParseException(String message) {
            super(message);
        }
    }

    public static class Payload {
        public final int v;
        public final String type;

        public Payload(int v, String type) {
            this.v = v;
            this.type = type;
        }
    }

    public static class ParseResult {
        public final boolean success;
        public final Payload payload;
        public final String errorMessage;

        private ParseResult(boolean success, Payload payload, String errorMessage) {
            this.success = success;
            this.payload = payload;
            this.errorMessage = errorMessage;
        }

        public static ParseResult success(Payload payload) {
            return new ParseResult(true, payload, null);
        }

        public static ParseResult error(String errorMessage) {
            return new ParseResult(false, null, errorMessage);
        }
    }
}
