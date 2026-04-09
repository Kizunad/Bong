package com.bong.client.hud;

import com.bong.client.state.NarrationState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongToastTest {
    @AfterEach
    void resetToastState() {
        BongToast.resetForTests();
    }

    @Test
    void warningToastUsesExpectedColorAndDuration() {
        BongToast.show(NarrationState.create("broadcast", null, "雷劫将至", "system_warning"), 1_000L);

        BongToast toast = BongToast.current(1_001L);
        assertFalse(toast.isEmpty());
        assertEquals("天道警示：雷劫将至", toast.text().getString());
        assertEquals(BongToast.WARNING_COLOR, toast.color());
        assertEquals(6_000L, toast.expiresAtMillis());
    }

    @Test
    void eraDecreeToastUsesExpectedColorAndDuration() {
        BongToast.show(NarrationState.create("broadcast", null, "仙门大开", "era_decree"), 500L);

        BongToast toast = BongToast.current(1_000L);
        assertFalse(toast.isEmpty());
        assertEquals("时代法旨：仙门大开", toast.text().getString());
        assertEquals(BongToast.ERA_DECREE_COLOR, toast.color());
        assertEquals(8_500L, toast.expiresAtMillis());
    }

    @Test
    void nonToastStylesDoNotReplaceCurrentToast() {
        BongToast.show(NarrationState.create("broadcast", null, "劫云逼近", "system_warning"), 0L);
        BongToast.show(NarrationState.create("broadcast", null, "风声微动", "perception"), 100L);

        BongToast toast = BongToast.current(200L);
        assertEquals("天道警示：劫云逼近", toast.text().getString());
        assertEquals(BongToast.WARNING_COLOR, toast.color());
    }

    @Test
    void toastExpiresAndClearsItself() {
        BongToast.show(NarrationState.create("broadcast", null, "气运逆流", "system_warning"), 100L);

        assertFalse(BongToast.current(5_099L).isEmpty());
        assertTrue(BongToast.current(5_100L).isEmpty());
        assertTrue(BongToast.current(5_101L).isEmpty());
    }

    @Test
    void buildCommandUsesClippedActiveToastText() {
        BongToast.show(
            NarrationState.create("broadcast", null, "这是一条会在运行时命令路径中被安全裁剪的超长天道警示文本。", "system_warning"),
            0L
        );

        HudRenderCommand command = BongToast.buildCommand(1_000L, text -> text == null ? 0 : text.length() * 6, 72).orElseThrow();

        assertTrue(command.isToast());
        assertTrue(command.text().endsWith("..."));
        assertTrue(command.text().startsWith("天道警示：") || command.text().startsWith("天道警示"));
        assertEquals(BongToast.WARNING_COLOR, command.color());
    }

    @Test
    void buildCommandRemainsAvailableUntilExpiryAfterNonToastNarration() {
        BongToast.show(NarrationState.create("broadcast", null, "劫云压境", "system_warning"), 0L);
        BongToast.show(NarrationState.create("broadcast", null, "风声微动", "perception"), 100L);

        HudRenderCommand command = BongToast.buildCommand(4_999L, text -> text == null ? 0 : text.length() * 6, 220).orElseThrow();

        assertNotNull(command);
        assertTrue(command.isToast());
        assertEquals("天道警示：劫云压境", command.text());
    }
}
