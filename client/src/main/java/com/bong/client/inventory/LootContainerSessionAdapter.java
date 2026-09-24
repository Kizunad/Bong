package com.bong.client.inventory;

import com.bong.client.hud.LootContainerStateStore;

import java.util.Objects;

/** 唯一负责把 HUD Store 会话转换成 inventory UI 会话的 adapter。 */
public final class LootContainerSessionAdapter {
    private LootContainerSessionAdapter() {
    }

    public static LootContainerSession from(LootContainerStateStore.Session session) {
        Objects.requireNonNull(session, "session must not be null");
        if (session instanceof LootContainerStateStore.OpenSession open) {
            return open(open);
        }
        LootContainerStateStore.Closed closed = (LootContainerStateStore.Closed) session;
        return new LootContainerSession.Closed(closed.sessionId(), closed.reason());
    }

    public static LootContainerSession.Open open(LootContainerStateStore.OpenSession session) {
        Objects.requireNonNull(session, "session must not be null");
        return new LootContainerSession.Open(
            session.sessionId(), session.sourceKind(), session.grade(),
            session.rows(), session.cols(), session.timeoutWallSecs(), session.placedItems()
        );
    }
}
