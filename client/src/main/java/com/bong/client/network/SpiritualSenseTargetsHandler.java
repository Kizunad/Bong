package com.bong.client.network;

import com.bong.client.dying_elder.DyingElderEncounterStore;
import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import com.bong.client.visual.realm_vision.PerceptionEdgeStateStore;
import com.bong.client.visual.realm_vision.SenseKind;
import com.bong.client.visual.realm_vision.SpiritualSenseStateReducer;

/**
 * Handles {@code spiritual_sense_targets} payloads from the server.
 *
 * <p>In addition to updating {@link PerceptionEdgeStateStore}, also derives the
 * SpiritEye active state for {@link DyingElderEncounterStore}: if the incoming
 * perception entries contain any entry with {@link SenseKind#SPIRIT_EYE}, the
 * player's spirit eye is considered active (true); otherwise false.
 *
 * <p>plan-dying-elder-v1 M1 fix: wires {@code DyingElderEncounterStore.setSpiritEyeActive}
 * to a real production call-site so the dying elder HUD correctly shows betrayal
 * probability when the player has spirit eye active, and "气息有异" when inactive.
 */
public final class SpiritualSenseTargetsHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        PerceptionEdgeState next = SpiritualSenseStateReducer.apply(envelope.payload());
        PerceptionEdgeStateStore.replace(next);

        // plan-dying-elder-v1 M1 fix: 派生 SpiritEye 激活状态供大能遭遇 HUD 使用。
        // 若感知条目中含有 SPIRIT_EYE 类型，则玩家已开天目（显示真实背叛概率）；否则显示"气息有异"。
        boolean spiritEyeActive = next.entries().stream()
            .anyMatch(e -> e.kind() == SenseKind.SPIRIT_EYE);
        DyingElderEncounterStore.setSpiritEyeActive(spiritEyeActive);

        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied spiritual_sense_targets (entries=" + next.entries().size()
                + ", generation=" + next.generation()
                + ", spiritEyeActive=" + spiritEyeActive + ")"
        );
    }
}
