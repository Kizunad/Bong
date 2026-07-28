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

    /** 清除当前连接派生的 loop flag；不影响任何长期 wiring。 */
    public static void clearOnDisconnect() {
        ACTIVE_FLAGS.clear();
    }

    /** @deprecated 仅保留给既有非 session 调用；断线清理请用 {@link #clearOnDisconnect()}. */
    @Deprecated(forRemoval = false)
    public static void clear() {
        clearOnDisconnect();
    }
}
