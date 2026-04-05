package com.bong.client;

import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

import java.nio.charset.StandardCharsets;

public class BongNetworkHandler {
    public static final int EXPECTED_VERSION = 1;

    public static void register() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "server_data"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = new String(bytes, StandardCharsets.UTF_8);
            ParseResult result = parseServerPayload(jsonPayload);

            if (result.success) {
                client.execute(() -> {
                    if (client.player != null) {
                        client.player.sendMessage(Text.literal("[Bong] " + result.payload.type + ": " + result.payload.message), false);
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
            if (fields.type == null || fields.message == null) {
                return ParseResult.error("Missing required fields 'type' or 'message'");
            }

            return ParseResult.success(new Payload(fields.version, fields.type, fields.message));
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
                case "message" -> fields.message = cursor.readQuotedString();
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
        private String message;
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

            if (input.charAt(index) == '"') {
                readQuotedString();
                return;
            }

            int start = index;
            while (index < input.length()) {
                char current = input.charAt(index);
                if (current == ',' || current == '}' || Character.isWhitespace(current)) {
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
        public final String message;

        public Payload(int v, String type, String message) {
            this.v = v;
            this.type = type;
            this.message = message;
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
