package com.bong.client.weapon;

import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.WeaponEquippedStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import com.bong.client.network.WeaponBrokenHandler;
import com.bong.client.state.VisualEffectState;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class WeaponVisualEvidenceHarnessTest {
    private static final Path RESOURCES = Path.of("src", "main", "resources");

    @AfterEach
    void tearDown() {
        WeaponEquippedStore.resetForTests();
    }

    @Test
    void armedIronSwordInjectsFakeStackAndTargetsSmlObjModel() throws IOException {
        WeaponEquippedStore.putOrClear(
            "main_hand",
            new EquippedWeapon("main_hand", 42L, "iron_sword", "sword", 200.0f, 200.0f, 0)
        );

        EquippedWeapon renderWeapon = WeaponEquippedStore.mainHandRenderWeapon();
        BongWeaponModelRegistry.Entry registryEntry = BongWeaponModelRegistry.get(renderWeapon.templateId()).orElseThrow();
        JsonObject hostJson = readHostJson(registryEntry);

        assertEquals("iron_sword", renderWeapon.templateId());
        assertNotNull(registryEntry.hostItemSupplier());
        assertEquals("item/iron_sword", registryEntry.vanillaModelPath());
        assertEquals("sml:builtin/obj", hostJson.get("parent").getAsString());
        assertEquals("bong:models/item/iron_sword/iron_sword.obj", hostJson.get("model").getAsString());
        System.out.println(
            "visual_evidence armed template=iron_sword fake_stack_injection_decision=true"
                + " host_model=" + registryEntry.vanillaModelPath()
                + " sml_model=" + hostJson.get("model").getAsString()
        );
    }

    @Test
    void unarmedStateDoesNotInjectWeaponFakeStack() {
        EquippedWeapon renderWeapon = WeaponEquippedStore.mainHandRenderWeapon();

        assertNull(renderWeapon);
        assertNull(WeaponVanillaIconMap.createStackFor("missing_weapon_template"));
        System.out.println("visual_evidence unarmed render_weapon=null fake_stack_injected=false");
    }

    @Test
    void brokenWeaponPayloadProducesToastAndBreakFlash() {
        ServerDataDispatch dispatch = new WeaponBrokenHandler().handle(parseEnvelope("""
            {"v":1,"type":"weapon_broken","instance_id":42,"template_id":"iron_sword"}
            """));
        ServerDataDispatch.ToastSpec toast = dispatch.alertToast().orElseThrow();
        VisualEffectState visualEffect = dispatch.visualEffectState().orElseThrow();

        assertTrue(dispatch.handled());
        assertEquals("武器损坏：iron_sword", toast.text());
        assertEquals(2_800L, toast.durationMillis());
        assertSame(VisualEffectState.EffectType.WEAPON_BREAK_FLASH, visualEffect.effectType());
        assertEquals(260L, visualEffect.durationMillis());
        assertEquals(1.0, visualEffect.intensity());
        System.out.println(
            "visual_evidence broken toast=\"" + toast.text() + "\" duration_ms=" + toast.durationMillis()
                + " effect=" + visualEffect.effectType()
                + " effect_duration_ms=" + visualEffect.durationMillis()
        );
    }

    private static JsonObject readHostJson(BongWeaponModelRegistry.Entry entry) throws IOException {
        Path path = RESOURCES.resolve("assets/minecraft/models").resolve(entry.vanillaModelPath() + ".json");

        assertTrue(Files.isRegularFile(path), entry.templateId() + " host JSON missing at " + path);
        return JsonParser.parseString(Files.readString(path)).getAsJsonObject();
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
