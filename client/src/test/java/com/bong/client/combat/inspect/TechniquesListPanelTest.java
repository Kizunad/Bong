package com.bong.client.combat.inspect;

import com.bong.client.inventory.model.MeridianChannel;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TechniquesListPanelTest {
    @Test
    void filtersByIdDisplayNameAndAlias() {
        var bengQuan = technique("burst_meridian.beng_quan", "崩拳", List.of("bq"));
        var severChain = technique("zhenmai.sever_chain", "绝脉断链", List.of("断链"));
        List<TechniquesListPanel.Technique> all = List.of(bengQuan, severChain);

        assertEquals(List.of(bengQuan), TechniquesListPanel.filter(all, "beng"));
        assertEquals(List.of(severChain), TechniquesListPanel.filter(all, "绝脉"));
        assertEquals(List.of(bengQuan), TechniquesListPanel.filter(all, "BQ"));
        assertEquals(all, TechniquesListPanel.filter(all, ""));
        assertTrue(TechniquesListPanel.filter(all, "不存在").isEmpty());
    }

    @Test
    void mapsTechniqueRequiredMeridiansToUiChannels() {
        var technique = new TechniquesListPanel.Technique(
            "zhenmai.sever_chain",
            "绝脉断链",
            TechniquesListPanel.Grade.YELLOW,
            1.0f,
            true,
            "",
            "",
            "",
            List.of(
                new TechniquesListPanel.RequiredMeridian("Pericardium", 0.7f),
                new TechniquesListPanel.RequiredMeridian("YinWei", 0.4f)
            ),
            0.4f,
            8,
            60,
            1.8f
        );

        assertEquals(
            List.of(MeridianChannel.PC, MeridianChannel.YIN_WEI),
            TechniquesListPanel.requiredChannels(technique)
        );
    }

    private static TechniquesListPanel.Technique technique(String id, String name, List<String> aliases) {
        return new TechniquesListPanel.Technique(
            id,
            name,
            aliases,
            TechniquesListPanel.Grade.MORTAL,
            0.0f,
            true,
            "",
            "",
            "",
            List.of(),
            0.0f,
            0,
            0,
            0.0f
        );
    }
}
