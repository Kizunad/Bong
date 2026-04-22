package com.bong.client.network;

import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class WeaponBrokenHandlerTest {
    @Test
    void returnsToastDispatchForBrokenWeapon() {
        ServerDataDispatch dispatch = new WeaponBrokenHandler().handle(parseEnvelope(
            "{" +
                "\"v\":1," +
                "\"type\":\"weapon_broken\"," +
                "\"instance_id\":42," +
                "\"template_id\":\"glass_sword\"}"
        ));

        assertTrue(dispatch.handled());
        ServerDataDispatch.ToastSpec toast = dispatch.alertToast().orElseThrow();
        assertEquals("武器损坏：glass_sword", toast.text());
        assertEquals(WeaponBrokenHandler.BROKEN_TOAST_COLOR, toast.color());
        assertEquals(WeaponBrokenHandler.BROKEN_TOAST_DURATION_MS, toast.durationMillis());
        assertSame(
            com.bong.client.state.VisualEffectState.EffectType.WEAPON_BREAK_FLASH,
            dispatch.visualEffectState().orElseThrow().effectType()
        );
        assertEquals(
            WeaponBrokenHandler.BROKEN_FLASH_DURATION_MS,
            dispatch.visualEffectState().orElseThrow().durationMillis()
        );
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json,
            json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
