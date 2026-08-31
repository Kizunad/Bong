package com.bong.client.combat.screen;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class DeathScreenXmlTest {
    @Test
    void finalWordsFormattingKeepsSixVisibleLines() {
        assertEquals(
            "「第一句」\n「第二句」\n「第三句」\n「第四句」\n「第五句」\n「第六句」",
            DeathScreen.formatFinalWords(List.of("第一句", "第二句", "第三句", "第四句", "第五句", "第六句", "第七句"))
        );
        assertEquals("", DeathScreen.formatFinalWords(List.of(" ", "\t")));
    }
}
