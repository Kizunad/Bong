package com.bong.client.forge.input;

import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.tsy.ExtractStateStore;
import net.minecraft.util.math.BlockPos;

import java.util.List;
import java.util.Map;

/**
 * plan-forge-session-entry-wiring-v1 §4.1 P1 —— {@code ForgeScreen}「起炉」入口的可测发送契约层。
 *
 * <p>对齐 {@code AlchemyScreen} I 键点燃范式（{@code sendAlchemyIgnite} 依赖已选中的
 * {@code RecipeScrollStore} 状态）：起炉同样依赖三份已收集状态——station pos（来自
 * {@code ForgeStationStore}，§4.1#3 新发现连带缺口：U 键全局打开的 {@code ForgeScreen} 本身
 * 没有交互上下文，必须从最近一次 {@code forge_station} S2C 快照里取）、当前选中的图谱 id
 * （来自 {@code BlueprintScrollStore}）、已点选的投料。三者任一缺失时静默不发送——UI 层负责
 * 用视觉提示引导玩家补齐前置条件，本层只守住「不完整就不发包」的契约。</p>
 */
public final class ForgeStartInputHandler {
    private ForgeStartInputHandler() {}

    /**
     * Forge global-open input policy. Extraction owns the shared legacy U path
     * while active, so Forge must consume but not dispatch that queued press.
     */
    public static boolean shouldDispatchForgeOpen() {
        return !ExtractStateStore.snapshot().extracting();
    }

    /**
     * 起炉模式判定（渲染与输入门禁共用同一语义，review #1141 major 收口）：
     * 无**活跃**会话即可起炉。会话结算后 server 最后一帧快照 active=false 但
     * sessionId 残留 &gt;0——按 sessionId 判定会把入口永久卡死（打完一炉再也
     * 点不动投料/I 键），必须按 active 判定。
     */
    public static boolean startModeAvailable(com.bong.client.forge.state.ForgeSessionStore.Snapshot session) {
        return !session.active();
    }

    /**
     * 尝试发起 {@code forge_start_session} 请求。
     *
     * @param stationPos  当前已知的砧坐标（{@code ForgeStationStore.snapshot().pos()}），
     *                    尚未收到任何 {@code forge_station} 快照时为 {@code null}
     * @param blueprintId 图谱书当前选中的图谱 id（{@code BlueprintScrollStore.current()}），
     *                    未学任何图谱时为 {@code null}
     * @param materials   已选投料：material id（对齐
     *                    {@link com.bong.client.inventory.model.InventoryItem#itemId()}）→ 数量，
     *                    同材料多个堆叠已在调用方聚合
     * @return 是否已发出请求；{@code false} 表示前置条件不完整、未发送任何包
     */
    public static boolean tryStartForge(BlockPos stationPos, String blueprintId, Map<String, Integer> materials) {
        if (stationPos == null) {
            return false;
        }
        if (blueprintId == null || blueprintId.isBlank()) {
            return false;
        }
        if (materials == null || materials.isEmpty()) {
            return false;
        }
        List<ClientRequestProtocol.ForgeMaterial> encoded = materials.entrySet().stream()
            .map(e -> new ClientRequestProtocol.ForgeMaterial(e.getKey(), e.getValue()))
            .toList();
        ClientRequestSender.sendForgeStartSession(stationPos, blueprintId, encoded);
        return true;
    }
}
