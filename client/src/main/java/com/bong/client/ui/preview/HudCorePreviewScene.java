package com.bong.client.ui.preview;

import com.bong.client.BongHud;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudTextureProbe;
import com.bong.client.hud.MiniBodyHudPlanner;
import com.bong.client.hud.QuickBarHudPlanner;
import com.bong.client.hud.svg.SvgHudBackend;
import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.PhysicalBody;
import com.bong.client.inventory.model.WoundLevel;
import com.bong.client.ui.contract.DefaultUiScreenScope;
import com.bong.client.ui.contract.UiScreenScope;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.texture.NativeImage;

import java.util.ArrayList;
import java.util.List;

/** 显式截图场景：用既有 Screen 作画布，调用真实 HUD planner 和 GUI 提交器。 */
final class HudCorePreviewScene implements UiPreviewScene {
    private final int phase;
    private UiScreenScope scope;
    private int renderedFrames;

    HudCorePreviewScene(int phase) {
        this.phase = phase;
    }

    @Override
    public void installFixture() {
        // 数值直接作为 planner 输入，不写网络 Store，不伪造正常联机玩家状态。
    }

    @Override
    public Screen createScreen() {
        renderedFrames = 0;
        scope = new DefaultUiScreenScope();
        return new GameMenuScreen(false);
    }

    @Override
    public void afterOpen(Screen screen) {
        UiScreenScope activeScope = scope;
        activeScope.onOpen();
        ScreenEvents.remove(screen).register(removed -> activeScope.close());
        ScreenEvents.afterRender(screen).register((current, context, mouseX, mouseY, delta) -> {
            activeScope.runIfOpen(() -> {
                MinecraftClient client = MinecraftClient.getInstance();
                // 此 Screen 的原内容已提交；独立画布只在预览白名单场景内存在。
                context.draw();
                context.getMatrices().push();
                context.getMatrices().translate(0, 0, 400);
                context.fill(0, 0, current.width, current.height, 0xFF172129);
                context.drawTextWithShadow(client.textRenderer,
                    "SVG HUD / " + label(), 12, 12, 0xFFE8E3D4);
                context.drawTextWithShadow(client.textRenderer,
                    current.width + " x " + current.height + " / " + templateId(), 12, 28, 0xFF9BB5BF);
                BongHud.drawCommandBatch(context, client, commands(current.width, current.height), SvgHudBackend.production());
                context.draw();
                context.getMatrices().pop();
                renderedFrames++;
            });
        });
    }

    private String label() {
        return switch (phase) {
            case 0 -> "满值 / 第一槽";
            case 1 -> "低真元 / 受伤 / 第二槽冷却 / 施法 25%";
            default -> "恢复 / 第三槽 / 冷却结束 / 施法 75%";
        };
    }

    private String templateId() {
        return "hud-core-" + phase;
    }

    private List<HudRenderCommand> commands(int width, int height) {
        float qi = phase == 0 ? 1.0f : phase == 1 ? 0.10f : 0.75f;
        float stamina = phase == 0 ? 1.0f : phase == 1 ? 0.35f : 0.65f;
        PhysicalBody body = phase == 1 ? PhysicalBody.builder()
            .wound(BodyPart.CHEST, WoundLevel.LACERATION)
            .wound(BodyPart.LEFT_CALF, WoundLevel.FRACTURE).build() : PhysicalBody.builder().build();
        long now = phase == 2 ? 4_000L : 2_000L;
        SkillBarConfig skills = SkillBarConfig.of(new SkillBarEntry[] {
            SkillBarEntry.skill("zhenmai.parry", "弹反", 4_000, 2_000,
                "bong-client:textures/gui/skill/zhenmai_parry.png"),
            SkillBarEntry.skill("zhenmai.harden", "护脉", 4_000, 2_000,
                "bong-client:textures/gui/skill/zhenmai_harden.png"),
            SkillBarEntry.skill("zhenmai.sever_chain", "断链", 4_000, 2_000,
                "bong-client:textures/gui/skill/zhenmai_sever_chain.png")
        }, new long[] {0L, phase == 0 ? 0L : 3_000L});
        CastState cast = phase == 0 ? CastState.idle()
            : CastState.casting(CastState.Source.SKILL_BAR, 0, 4_000, 1_000L);
        List<HudRenderCommand> commands = new ArrayList<>(MiniBodyHudPlanner.buildCommands(
            CombatHudState.create(1.0f, qi, stamina, DerivedAttrFlags.none()), body, null, now, width, height));
        commands.addAll(QuickBarHudPlanner.buildCommands(QuickSlotConfig.empty(),
            skills, phase, cast, List.<InventoryItem>of(), now, width, height, HudTextureProbe::exists));
        return List.copyOf(commands);
    }

    @Override public String selectedTemplateId(Screen screen) { return templateId(); }
    @Override public boolean isReady(Screen screen) { return screen.width > 0 && screen.height > 0; }
    @Override public boolean initializationFailed(Screen screen) { return false; }

    @Override
    public void validateGeometry(Screen screen, UiPreviewShot shot) {
        if (renderedFrames == 0) {
            throw new IllegalStateException("HUD 尚未实际渲染，不能把加载画面作为截图证据");
        }
        for (HudRenderCommand command : commands(screen.width, screen.height)) {
            if (!(command.isVector() || command.isRect() || command.isTexturedRect())) continue;
            if (command.x() < 0 || command.y() < 0 || command.x() + command.width() > screen.width
                || command.y() + command.height() > screen.height) {
                throw new IllegalStateException("HUD 图形超出视口: " + command.layer() + "/" + command.text());
            }
        }
    }

    @Override
    public void validateImage(Screen screen, NativeImage image) {
        // Minecraft NativeImage 使用 ABGR；中心画布用于识别加载遮罩或错误屏幕。
        int background = pixel(image, screen, screen.width / 2, screen.height / 2);
        if ((background & 0xFFFFFF) != 0x292117) {
            throw new IllegalStateException("截图未显示 HUD 画布，可能仍被加载遮罩覆盖");
        }
        HudRenderCommand body = commands(screen.width, screen.height).stream()
            .filter(c -> c.isVector() && c.text().equals("body")).findFirst().orElseThrow();
        int head = pixel(image, screen, body.x() + body.width() / 2, body.y() + 4);
        if ((head & 0xFF) < 70 || ((head >>> 8) & 0xFF) < 70 || ((head >>> 16) & 0xFF) < 70) {
            throw new IllegalStateException("人体 SVG 没有进入 framebuffer，不能只凭 planner 几何通过");
        }
    }

    private static int pixel(NativeImage image, Screen screen, int x, int y) {
        return image.getColor((int) ((x + 0.5) * image.getWidth() / screen.width),
            (int) ((y + 0.5) * image.getHeight() / screen.height));
    }

    @Override
    public void cleanup() {
        if (scope != null) {
            scope.close();
        }
    }
}
