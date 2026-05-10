package com.bong.client.environment;

import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

public final class EnvironmentAudioLoopState {
    private static final Set<String> ACTIVE_FLAGS = ConcurrentHashMap.newKeySet();

    private EnvironmentAudioLoopState() {
    }

    public static void activate(String flag) {
        if (flag != null && !flag.isBlank()) {
            ACTIVE_FLAGS.add(flag);
        }
    }

    public static void deactivate(String flag) {
        if (flag != null) {
            ACTIVE_FLAGS.remove(flag);
        }
    }

    public static boolean isActive(String flag) {
        return flag != null && ACTIVE_FLAGS.contains(flag);
    }

    public static void clear() {
        ACTIVE_FLAGS.clear();
    }
}
