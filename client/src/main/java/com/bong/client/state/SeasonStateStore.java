package com.bong.client.state;

import java.util.concurrent.atomic.AtomicReference;

public final class SeasonStateStore {
    private static final AtomicReference<SeasonState> STATE =
        new AtomicReference<>(SeasonState.summerAt(0L));

    private SeasonStateStore() {
    }

    public static SeasonState snapshot() {
        return STATE.get();
    }

    public static void replace(SeasonState next) {
        STATE.set(next == null ? SeasonState.summerAt(0L) : next);
    }

    /** 断线时恢复无服务端 season payload 的夏季基线。 */
    public static void clearOnDisconnect() {
        STATE.set(SeasonState.summerAt(0L));
    }

    public static void resetForTests() {
        STATE.set(SeasonState.summerAt(0L));
    }
}
