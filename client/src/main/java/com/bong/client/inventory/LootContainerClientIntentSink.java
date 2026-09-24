package com.bong.client.inventory;

import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;

import java.util.Objects;

/** 搜刮动作的 C2S adapter；Screen/panel 不再创建 network protocol location。 */
public final class LootContainerClientIntentSink implements UiIntentSink<LootContainerIntent> {
    private final Transport transport;

    LootContainerClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    public static LootContainerClientIntentSink production() {
        return new LootContainerClientIntentSink(new Transport() {
            @Override
            public void move(long sessionId, long itemInstanceId, String fromContainer, int fromRow, int fromCol,
                             String toContainer, int toRow, int toCol) {
                ClientRequestSender.sendExternalContainerMove(
                    sessionId,
                    itemInstanceId,
                    new ClientRequestProtocol.ContainerLoc(fromContainer, fromRow, fromCol),
                    new ClientRequestProtocol.ContainerLoc(toContainer, toRow, toCol)
                );
            }

            @Override public void close(long sessionId) { ClientRequestSender.sendExternalContainerClose(sessionId); }
        });
    }

    @Override
    public UiIntentResult dispatch(LootContainerIntent intent) {
        if (intent == null) return UiIntentResult.rejected("loot intent must not be null");
        try {
            if (intent instanceof LootContainerIntent.Close close) {
                requireSession(close.sessionId());
                transport.close(close.sessionId());
                return UiIntentResult.accepted();
            }
            LootContainerIntent.Move move = (LootContainerIntent.Move) intent;
            requireSession(move.sessionId());
            if (move.itemInstanceId() <= 0L) throw new IllegalArgumentException("item instance_id must be > 0");
            requireLocation(move.fromContainer(), move.fromRow(), move.fromCol(), "from");
            requireLocation(move.toContainer(), move.toRow(), move.toCol(), "to");
            transport.move(move.sessionId(), move.itemInstanceId(), move.fromContainer(), move.fromRow(), move.fromCol(),
                move.toContainer(), move.toRow(), move.toCol());
            return UiIntentResult.accepted();
        } catch (IllegalArgumentException failure) {
            return UiIntentResult.rejected(failure.getMessage());
        } catch (RuntimeException failure) {
            String detail = failure.getMessage();
            return UiIntentResult.error("loot transport failed: "
                + (detail == null || detail.isBlank() ? failure.getClass().getSimpleName() : detail));
        }
    }

    private static void requireSession(long sessionId) {
        if (sessionId <= 0L) throw new IllegalArgumentException("session id must be > 0");
    }

    private static void requireLocation(String container, int row, int col, String name) {
        if (container == null || container.isBlank()) throw new IllegalArgumentException(name + " container must not be blank");
        if (row < 0 || col < 0) throw new IllegalArgumentException(name + " row/col must be >= 0");
    }

    interface Transport {
        void move(long sessionId, long itemInstanceId, String fromContainer, int fromRow, int fromCol,
                  String toContainer, int toRow, int toCol);
        void close(long sessionId);
    }
}
