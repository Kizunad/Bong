package com.bong.client.inventory;

import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;

/** 库存领域的移动意图边界；UI 只提交已校验的来源、目标和数量。 */
public final class InventoryMoveService {
    private InventoryMoveService() {}

    public static boolean move(
        long instanceId,
        ClientRequestProtocol.InvLocation from,
        ClientRequestProtocol.InvLocation to,
        boolean rotated
    ) {
        return ClientRequestSender.sendInventoryMove(instanceId, from, to, rotated);
    }

    public static boolean move(
        long instanceId,
        ClientRequestProtocol.InvLocation from,
        ClientRequestProtocol.InvLocation to,
        boolean rotated,
        int count
    ) {
        return ClientRequestSender.sendInventoryMove(instanceId, from, to, rotated, count);
    }
}
