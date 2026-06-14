package com.bong.client.interaction;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.network.ClientRequestProtocol;

/**
 * Maps a held inventory item to its zhenfa / qi-scatter-bead interaction intent.
 *
 * <p>This helper deliberately lives OUTSIDE the {@code com.bong.client.mixin} package:
 * Mixin owns that package and forbids regular code from referencing classes inside it
 * directly — touching such a class from a mixin handler throws
 * {@link org.spongepowered.asm.mixin.transformer.throwables.IllegalClassLoadError} at
 * runtime (it crashed the client on the first alchemy block interaction). A plain
 * resolver must therefore be a normal class in a non-mixin package.
 */
public final class ClientInteractionItemResolver {
    private static final String WARNING_TRAP_ITEM_ID = "warning_trap";
    private static final String BLAST_TRAP_ITEM_ID = "blast_trap";
    private static final String SLOW_TRAP_ITEM_ID = "slow_trap";
    private static final String BEAST_TRAP_ITEM_ID = "beast_trap";
    private static final String TRIP_WIRE_ITEM_ID = "trip_wire";
    private static final String BAIT_STAKE_ITEM_ID = "bait_stake";
    private static final String GATHER_ARRAY_BASE_ITEM_ID = "gather_array_base";
    private static final String QI_SCATTER_BEAD_ITEM_ID = "qi_scatter_bead";
    private static final String ARRAY_FLAG_BASIC_ITEM_ID = "array_flag_basic";
    private static final String ARRAY_EYE_BASIC_ITEM_ID = "array_eye_basic";

    private ClientInteractionItemResolver() {
    }

    public static ClientRequestProtocol.ZhenfaKind zhenfaKindForItem(InventoryItem item) {
        if (item == null) return null;
        return switch (item.itemId()) {
            case WARNING_TRAP_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.WARNING_TRAP;
            case BLAST_TRAP_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.BLAST_TRAP;
            case SLOW_TRAP_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.SLOW_TRAP;
            case BEAST_TRAP_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.BEAST_TRAP;
            case TRIP_WIRE_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.TRIP_WIRE;
            case BAIT_STAKE_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.DECOY_STAKE;
            case GATHER_ARRAY_BASE_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.LINGJU;
            case ARRAY_FLAG_BASIC_ITEM_ID, ARRAY_EYE_BASIC_ITEM_ID -> ClientRequestProtocol.ZhenfaKind.NETWORK_ARRAY;
            default -> null;
        };
    }

    public static Long qiScatterBeadUseInstanceId(InventoryItem item) {
        if (item == null || !QI_SCATTER_BEAD_ITEM_ID.equals(item.itemId()) || item.instanceId() <= 0) {
            return null;
        }
        return item.instanceId();
    }
}
