package com.bong.client.combat.screen;

import com.bong.client.combat.DeathIntent;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.ui.intent.UiIntentResult;
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

    @Test
    void successfulRetryClearsPriorFailureFeedback() {
        int[] attempts = {0};
        DeathScreen screen = new DeathScreen(
            DeathStateStore.State.HIDDEN,
            ignored -> attempts[0]++ == 0
                ? UiIntentResult.rejected("暂不可操作")
                : UiIntentResult.accepted()
        );

        screen.dispatch(new DeathIntent.Reincarnate());
        assertEquals("操作未提交: 暂不可操作", screen.feedbackTextForTests());

        screen.dispatch(new DeathIntent.Reincarnate());
        assertEquals("", screen.feedbackTextForTests(),
            "重试成功必须清除上一次失败反馈，避免界面继续显示过期错误");
    }
}
