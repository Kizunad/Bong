package com.bong.client.practice;

import com.bong.client.combat.SkillConfigStore;
import com.bong.client.combat.inspect.SkillConfigSchemaRegistry;
import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.inventory.model.*;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.inventory.state.RaceGateEval;

/** 本地展示准入；服务端仍负责最终校验。 */
public final class TechniqueAvailability {
    private TechniqueAvailability() {}
    public static String reason(TechniquesListPanel.Technique technique) {
        if (technique == null) return "";
        // plan-race-system-v1 P3c — 功法门（required_race）判**本体**身份（race_id /
        // intrinsic_is_humanoid），与装备门（判当前形态）不同轴。fail-closed：有 gate 但
        // 本体身份未到时也置灰。server 习得/施放门权威，此处仅预览灰。
        if (RaceGateEval.isTechniqueBlocked(technique.id())) {
            return "本体种族不符";
        }
        String configReason = SkillConfigSchemaRegistry.missingRequiredReason(
            technique.id(),
            SkillConfigStore.configFor(technique.id())
        );
        if (!configReason.isBlank()) return configReason;
        if (!technique.active()) return "尚未激活";
        MeridianBody body = MeridianStateStore.snapshot();
        if (body == null) return "";
        for (TechniquesListPanel.RequiredMeridian required : technique.requiredMeridians()) {
            MeridianChannel channel = TechniquesListPanel.channelFromWire(required.channel()).orElse(null);
            if (channel == null) continue;
            ChannelState state = body.channel(channel);
            if (state == null) continue;
            if (state.damage() == ChannelState.DamageLevel.SEVERED) {
                return channel.displayName() + "已断";
            }
            if (state.blocked()) {
                return channel.displayName() + "已封闭";
            }
            double health = state.capacity() <= 0 ? 0.0 : state.effectiveFlow() / state.capacity();
            if (health < required.minHealth()) {
                return channel.displayName() + "健康不足";
            }
        }
        return "";
    }

}
