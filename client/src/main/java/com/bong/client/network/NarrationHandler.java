package com.bong.client.network;

import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.state.NarrationState;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import net.minecraft.text.MutableText;
import net.minecraft.text.Text;
import net.minecraft.util.Formatting;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class NarrationHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonArray narrations = readNarrations(envelope.payload());
        if (narrations == null || narrations.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring narration payload because field 'narrations' is missing, not an array, or empty"
            );
        }

        List<ParsedNarration> validNarrations = new ArrayList<>();
        int invalidEntries = 0;
        for (JsonElement narrationElement : narrations) {
            ParsedNarration parsedNarration = parseNarration(narrationElement);
            if (parsedNarration == null) {
                invalidEntries++;
                continue;
            }
            validNarrations.add(parsedNarration);
        }

        if (validNarrations.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring narration payload because field 'narrations' contained no valid entries"
            );
        }

        List<Text> chatMessages = validNarrations.stream()
            .filter(parsed -> parsed.chatText() != null)
            .map(ParsedNarration::chatText)
            .toList();
        NarrationState latestNarrationState = validNarrations.get(validNarrations.size() - 1).state();
        NarrationState latestToastNarrationState = latestToastNarration(validNarrations);

        StringBuilder logMessage = new StringBuilder()
            .append("Routed narration payload with ")
            .append(validNarrations.size())
            .append(" valid entries");
        if (latestToastNarrationState != null) {
            logMessage.append(" and central toast trigger");
        }
        if (invalidEntries > 0) {
            logMessage.append("; ignored ").append(invalidEntries).append(" invalid entries");
        }

        return ServerDataDispatch.handledWithNarration(
            envelope.type(),
            chatMessages,
            latestNarrationState,
            latestToastNarrationState,
            logMessage.toString()
        );
    }

    private static JsonArray readNarrations(JsonObject payload) {
        JsonElement narrationsElement = payload.get("narrations");
        if (narrationsElement == null || narrationsElement.isJsonNull() || !narrationsElement.isJsonArray()) {
            return null;
        }
        return narrationsElement.getAsJsonArray();
    }

    private static ParsedNarration parseNarration(JsonElement narrationElement) {
        if (narrationElement == null || narrationElement.isJsonNull() || !narrationElement.isJsonObject()) {
            return null;
        }

        JsonObject narrationObject = narrationElement.getAsJsonObject();
        String scopeName = readOptionalString(narrationObject, "scope");
        String target = readOptionalString(narrationObject, "target");
        String text = readOptionalString(narrationObject, "text");
        String styleName = readOptionalString(narrationObject, "style");

        NarrationState narrationState = NarrationState.create(scopeName, target, text, styleName);
        if (narrationState.isEmpty()) {
            return null;
        }

        return new ParsedNarration(
            narrationState,
            chatTextFor(narrationState, isKnownStyle(styleName))
        );
    }

    private static String readOptionalString(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }

        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isString()) {
            return null;
        }
        return primitive.getAsString();
    }

    private static NarrationState latestToastNarration(List<ParsedNarration> validNarrations) {
        for (int index = validNarrations.size() - 1; index >= 0; index--) {
            NarrationState narrationState = validNarrations.get(index).state();
            if (narrationState.isToastEligible()) {
                return narrationState;
            }
        }
        return null;
    }

    static Text createChatText(NarrationState narrationState, boolean knownStyle) {
        if (!knownStyle) {
            return Text.literal(narrationState.text());
        }

        return switch (narrationState.style()) {
            case SYSTEM_WARNING -> prefixedText("[天道警示] ", Formatting.RED, true, narrationState.text());
            case PERCEPTION -> prefixedText("[感知] ", Formatting.GRAY, false, narrationState.text());
            case NARRATION -> prefixedText("[叙事] ", Formatting.WHITE, false, narrationState.text());
            case ERA_DECREE -> prefixedText("[时代法旨] ", Formatting.GOLD, true, narrationState.text());
            case POLITICAL_JIANGHU -> prefixedText("[江湖传闻] ", Formatting.DARK_AQUA, false, narrationState.text());
        };
    }

    private static Text chatTextFor(NarrationState narrationState, boolean knownStyle) {
        if (isZonePoliticalNarration(narrationState)) {
            UnifiedEventStore.stream().publish(
                UnifiedEvent.Channel.SOCIAL,
                UnifiedEvent.Priority.P2_NORMAL,
                "political_jianghu:" + narrationState.target().orElse("zone"),
                "[江湖传闻] " + narrationState.text(),
                UnifiedEvent.Channel.SOCIAL.defaultColor(),
                System.currentTimeMillis()
            );
            return null;
        }
        return createChatText(narrationState, knownStyle);
    }

    private static boolean isZonePoliticalNarration(NarrationState narrationState) {
        return narrationState.style() == NarrationState.Style.POLITICAL_JIANGHU
            && narrationState.scope() == NarrationState.Scope.ZONE;
    }

    private static Text prefixedText(String prefix, Formatting formatting, boolean boldPrefix, String body) {
        MutableText prefixText = Text.literal(prefix).formatted(formatting);
        if (boldPrefix) {
            prefixText.formatted(Formatting.BOLD);
        }

        return prefixText.append(Text.literal(body).formatted(formatting));
    }

    private static boolean isKnownStyle(String styleName) {
        if (styleName == null) {
            return false;
        }

        String normalizedStyleName = styleName.trim().toLowerCase(Locale.ROOT);
        for (NarrationState.Style style : NarrationState.Style.values()) {
            if (style.wireName().equals(normalizedStyleName)) {
                return true;
            }
        }
        return false;
    }

    private record ParsedNarration(NarrationState state, Text chatText) {
    }
}
