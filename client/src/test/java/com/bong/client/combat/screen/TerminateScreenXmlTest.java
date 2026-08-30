package com.bong.client.combat.screen;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class TerminateScreenXmlTest {
    @Test
    void finalWordsFormattingKeepsOnlyVisibleLines() {
        assertEquals("第一句\n第二句", TerminateScreen.formatFinalWords("第一句\n\n第二句\n"));
        assertEquals("", TerminateScreen.formatFinalWords(" \n\t"));
    }

    @Test
    void archetypeLabelDoesNotRenderAnEmptySuggestion() {
        assertEquals("", TerminateScreen.formatArchetype(" "));
        assertEquals("建议新角色原型: 游侠", TerminateScreen.formatArchetype("游侠"));
    }
}
