package com.bong.client.combat.screen;

import com.bong.client.network.ClientRequestSender;
import com.google.gson.JsonObject;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;

/**
 * 武器/法宝修复界面 (plan §U-parallel / §2.4). Presents current weapon
 * durability and a queue of repair materials; commits via {@code combat.repair_weapon}.
 */
public final class RepairScreen extends Screen {
    public static final int BG_COLOR = 0xC0182010;
    public static final int TITLE_COLOR = 0xFF80FF80;
    public static final int TEXT_COLOR = 0xFFE0E0E0;

    private final float durabilityNorm;
    private final String weaponLabel;
    private final long weaponInstanceId;
    private final int stationX;
    private final int stationY;
    private final int stationZ;

    public RepairScreen(String weaponLabel, float durabilityNorm, long weaponInstanceId, int stationX, int stationY, int stationZ) {
        super(Text.literal("\u517b\u62a4"));
        this.weaponLabel = weaponLabel == null ? "-" : weaponLabel;
        this.durabilityNorm = Math.max(0f, Math.min(1f, durabilityNorm));
        this.weaponInstanceId = Math.max(0L, weaponInstanceId);
        this.stationX = stationX;
        this.stationY = stationY;
        this.stationZ = stationZ;
    }

    @Override public boolean shouldPause() { return true; }

    @Override
    protected void init() {
        super.init();
        int cx = width / 2;
        int y = height / 2 + 20;

        this.addDrawableChild(ButtonWidget.builder(
            Text.literal("\u6295\u5165\u7cbe\u94a2\u9501"),
            b -> commit("refined_steel")
        ).dimensions(cx - 110, y, 100, 20).build());
        this.addDrawableChild(ButtonWidget.builder(
            Text.literal("\u6295\u5165\u4e39\u836f"),
            b -> commit("pill")
        ).dimensions(cx + 10, y, 100, 20).build());
    }

    private void commit(String material) {
        if (weaponInstanceId > 0L) {
            ClientRequestSender.sendRepairWeapon(weaponInstanceId, stationX, stationY, stationZ);
        } else {
            JsonObject p = new JsonObject();
            p.addProperty("material", material);
            ClientRequestSender.send("combat.repair_weapon", p);
        }
        this.close();
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        context.fill(0, 0, width, height, BG_COLOR);
        context.drawCenteredTextWithShadow(this.textRenderer, "\u517b\u62a4 \u00b7 " + weaponLabel, width / 2, height / 2 - 60, TITLE_COLOR);

        int barW = 200;
        int barX = (width - barW) / 2;
        int barY = height / 2 - 20;
        context.fill(barX, barY, barX + barW, barY + 6, 0xFF303030);
        int fill = Math.max(0, Math.round(durabilityNorm * barW));
        int color = durabilityNorm < 0.3f ? 0xFFE04040 : (durabilityNorm < 0.7f ? 0xFFE0C040 : 0xFF60D060);
        if (fill > 0) context.fill(barX, barY, barX + fill, barY + 6, color);
        context.drawCenteredTextWithShadow(
            this.textRenderer,
            "\u8010\u4e45: " + Math.round(durabilityNorm * 100) + "%",
            width / 2, barY - 12, TEXT_COLOR
        );
        super.render(context, mouseX, mouseY, delta);
    }

    public float durabilityNormForTests() { return durabilityNorm; }
    public String weaponLabelForTests() { return weaponLabel; }
    public long weaponInstanceIdForTests() { return weaponInstanceId; }
    public int stationXForTests() { return stationX; }
    public int stationYForTests() { return stationY; }
    public int stationZForTests() { return stationZ; }
}
