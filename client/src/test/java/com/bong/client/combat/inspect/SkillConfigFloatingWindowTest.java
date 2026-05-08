package com.bong.client.combat.inspect;

import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class SkillConfigFloatingWindowTest {
    @Test
    void renderModelChoosesEnumMeridianAndBoolControls() {
        var schema = new SkillConfigSchemaRegistry.SkillConfigSchema(
            "test.skill",
            List.of(
                SkillConfigSchemaRegistry.ConfigField.enumeration(
                    "mode",
                    "模式",
                    List.of(
                        new SkillConfigSchemaRegistry.Option("a", "甲"),
                        new SkillConfigSchemaRegistry.Option("b", "乙")
                    ),
                    true,
                    "a"
                ),
                SkillConfigSchemaRegistry.ConfigField.meridian(
                    "meridian_id",
                    "经脉",
                    List.of(new SkillConfigSchemaRegistry.Option("Pericardium", "心包经 Pericardium")),
                    true,
                    "Pericardium"
                ),
                SkillConfigSchemaRegistry.ConfigField.bool("confirm", "确认", false, true)
            )
        );
        JsonObject config = new JsonObject();
        config.addProperty("mode", "b");

        List<SkillConfigFloatingWindow.RenderField> fields =
            SkillConfigFloatingWindow.renderFields(schema, config);

        assertEquals(SkillConfigFloatingWindow.ControlKind.ENUM, fields.get(0).controlKind());
        assertEquals("b", fields.get(0).currentValue());
        assertEquals(SkillConfigFloatingWindow.ControlKind.MERIDIAN_ID, fields.get(1).controlKind());
        assertEquals("Pericardium", fields.get(1).currentValue());
        assertEquals(SkillConfigFloatingWindow.ControlKind.BOOL, fields.get(2).controlKind());
        assertEquals("true", fields.get(2).currentValue());
    }

    @Test
    void zhenmaiFixtureExposesExpectedFields() {
        var schema = SkillConfigSchemaRegistry.schemaFor("zhenmai.sever_chain").orElseThrow();
        List<SkillConfigFloatingWindow.RenderField> fields =
            SkillConfigFloatingWindow.renderFields(schema, null);

        assertEquals("meridian_id", fields.get(0).key());
        assertEquals(SkillConfigFloatingWindow.ControlKind.MERIDIAN_ID, fields.get(0).controlKind());
        assertEquals(20, fields.get(0).options().size());
        assertEquals("backfire_kind", fields.get(1).key());
        assertEquals(SkillConfigFloatingWindow.ControlKind.ENUM, fields.get(1).controlKind());
        assertEquals("real_yuan", fields.get(1).currentValue());
    }
}
