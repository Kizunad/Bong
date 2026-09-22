package com.bong.client.forge;

import com.bong.client.network.ClientRequestProtocol;
import net.minecraft.util.math.BlockPos;
import java.util.List;

/** 锻造窗口的领域动作，不包含窗口布局操作。 */
public sealed interface ForgeIntent extends com.bong.client.ui.contract.UiIntent {
    record Start(BlockPos station, String blueprint, List<ClientRequestProtocol.ForgeMaterial> materials) implements ForgeIntent {
        public Start { materials = List.copyOf(materials); }
    }
    record TurnPage(int delta) implements ForgeIntent {}
    record Material(BlockPos station, String blueprint, Long instanceId, boolean returning, long revision) implements ForgeIntent {}
    record Advance(long session) implements ForgeIntent {}
    record Hit(long session, ClientRequestProtocol.TemperBeat beat) implements ForgeIntent {}
    record Inscribe(long session, String inscription) implements ForgeIntent {}
    record Inject(long session, double qi) implements ForgeIntent {}
}
