package com.bong.client.input;

import net.minecraft.client.MinecraftClient;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;

public final class InteractKeyRouter {
    private static InteractKeyRouter global = new InteractKeyRouter();

    private final List<IntentHandler> handlers = new ArrayList<>();

    public static InteractKeyRouter global() {
        return global;
    }

    public static void resetGlobalForTests() {
        global = new InteractKeyRouter();
    }

    public synchronized void register(IntentHandler handler) {
        if (handler == null) {
            return;
        }
        handlers.add(handler);
    }

    public synchronized int handlerCountForTests() {
        return handlers.size();
    }

    public boolean route(MinecraftClient client) {
        boolean screenOpen = client != null && client.currentScreen != null;
        return route(client, screenOpen);
    }

    public boolean route(MinecraftClient client, boolean screenOpen) {
        if (screenOpen) {
            return false;
        }

        List<HandlerCandidate> candidates = new ArrayList<>();
        List<IntentHandler> snapshot;
        synchronized (this) {
            snapshot = List.copyOf(handlers);
        }
        for (IntentHandler handler : snapshot) {
            Optional<InteractCandidate> candidate = handler.candidate(client);
            candidate.ifPresent(value -> candidates.add(new HandlerCandidate(handler, value)));
        }

        Optional<InteractCandidate> winner = InteractPriorityResolver.choose(
            candidates.stream().map(HandlerCandidate::candidate).toList()
        );
        if (winner.isEmpty()) {
            return false;
        }

        for (HandlerCandidate candidate : candidates) {
            if (candidate.candidate() == winner.get()) {
                return candidate.handler().dispatch(client, candidate.candidate());
            }
        }
        return false;
    }

    private record HandlerCandidate(IntentHandler handler, InteractCandidate candidate) {
    }
}
