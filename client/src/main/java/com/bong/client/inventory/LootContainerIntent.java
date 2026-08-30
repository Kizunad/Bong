package com.bong.client.inventory;

import com.bong.client.ui.contract.UiIntent;

/** 搜刮屏 typed actions，位置使用语义字段而非 ClientRequestProtocol 类型。 */
public sealed interface LootContainerIntent extends UiIntent permits
    LootContainerIntent.Move,
    LootContainerIntent.Close {

    record Move(
        long sessionId,
        long itemInstanceId,
        String fromContainer,
        int fromRow,
        int fromCol,
        String toContainer,
        int toRow,
        int toCol
    ) implements LootContainerIntent {}

    record Close(long sessionId) implements LootContainerIntent {}
}
