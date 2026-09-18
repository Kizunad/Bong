package com.bong.client.forge;

import com.bong.client.forge.screen.InscriptionPanelComponent;
import com.bong.client.forge.screen.TemperingTrackComponent;
import com.bong.client.inspect.ItemInspectModel;
import com.bong.client.ui.model.ModelPreviewCatalog;
import com.bong.client.ui.model.ModelPreviewComponent;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import net.minecraft.client.MinecraftClient;
import net.minecraft.sound.SoundEvents;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Vec3d;

/** 实际装备模型与工序舞台；动画只投影权威状态，不参与成败判定。 */
public final class ForgeWorkbenchComponent extends ModelPreviewComponent {
    private static final Identifier BACKDROP = new Identifier("bong-client", "textures/gui/forge/workbench.png");
    private final ForgeWindows windows;
    private String itemId = "";
    private String phase = "";
    private long lastOutcome;
    private long phaseStarted = System.nanoTime();
    private int lastBeat;
    private long lastImpact;

    public ForgeWorkbenchComponent(ForgeWindows windows) {
        this.windows = windows;
        id("forge-workbench");
        lastOutcome = windows.outcome().sessionId();
    }

    public void refresh() {
        var model = windows.model();
        var outcome = windows.outcome();
        var blueprint = outcome.sessionId() > 0 ? model.blueprints().stream()
            .filter(entry -> entry.id().equals(outcome.blueprintId())).findFirst().orElse(model.blueprint())
            : model.blueprint();
        String nextItem = outcome.weaponItem() != null && outcome.sessionId() > 0 ? outcome.weaponItem()
            : blueprint == null ? "" : blueprint.outputItem();
        if (!nextItem.equals(itemId)) {
            itemId = nextItem;
            option(new ModelPreviewCatalog.Entry(itemId, itemId, ModelPreviewCatalog.Category.ITEM, null,
                () -> ItemInspectModel.find(itemId).orElseThrow(() -> new IllegalStateException("此装备尚无模型：" + itemId))));
        }
        String nextPhase = outcome.sessionId() > 0 ? outcome.bucket()
            : model.session().active() ? model.session().currentStep() : "prepare";
        if (!nextPhase.equals(phase)) {
            phase = nextPhase;
            phaseStarted = System.nanoTime();
            camera().focus(new Vec3d(.5, .5, .5), 1, 25, -12, true);
        }
        material(!phase.equals("waste") && !phase.equals("explode"));
        int beat = TemperingTrackComponent.renderStateFrom(model.session()).beatCursor();
        if (phase.equals("tempering") && beat > lastBeat) {
            lastImpact = System.nanoTime();
            sound(SoundEvents.BLOCK_ANVIL_USE, .25f, 1.05f);
        }
        lastBeat = beat;
        if (outcome.sessionId() > 0 && outcome.sessionId() != lastOutcome) {
            lastOutcome = outcome.sessionId();
            switch (outcome.bucket()) {
                case "perfect" -> sound(SoundEvents.BLOCK_ENCHANTMENT_TABLE_USE, .4f, 1.3f);
                case "good" -> sound(SoundEvents.BLOCK_ANVIL_USE, .3f, 1.4f);
                case "flawed" -> sound(SoundEvents.BLOCK_ANVIL_LAND, .3f, .8f);
                case "explode" -> sound(SoundEvents.ENTITY_GENERIC_EXPLODE, .3f, 1.1f);
                default -> sound(SoundEvents.BLOCK_FIRE_EXTINGUISH, .3f, .8f);
            }
        }
    }

    @Override protected void drawBackdrop(OwoUIDrawContext context) {
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        // 等比 cover，避免窗口改尺寸把砧台拉长。
        float scale = Math.max(width / 768f, height / 512f);
        int imageWidth = Math.round(768 * scale);
        int imageHeight = Math.round(512 * scale);
        context.enableScissor(x, y, x + width, y + height);
        try {
            context.drawTexture(BACKDROP, x + (width - imageWidth) / 2, y + (height - imageHeight) / 2,
                imageWidth, imageHeight, 0, 0, 768, 512, 768, 512);
            context.fillGradient(x, y, x + width, y + height / 3, 0xC00C0C0E, 0x000C0C0E);
            context.fillGradient(x, y + height * 3 / 4, x + width, y + height, 0x000C0C0E, 0xEA0C0C0E);
            if (phase.equals("billet")) {
                int alpha = 32 + (int) (12 * Math.sin(seconds() * 3));
                context.fillGradient(x, y + height / 2, x + width, y + height, 0, alpha << 24 | 0xC65B23);
            }
        } finally {
            context.disableScissor();
        }
        context.drawRectOutline(x, y, width, height, 0xFF695341);
    }

    @Override public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        if (itemId.isEmpty()) drawBackdrop(context);
        else super.draw(context, mouseX, mouseY, partialTicks, delta);
        context.draw();
        context.enableScissor(x + 1, y + 1, x + width - 1, y + height - 1);
        try {
            float time = seconds();
            if (phase.equals("billet") || phase.equals("tempering")) embers(context, time);
            if (phase.equals("inscription")) inscription(context, time);
            if (phase.equals("consecration")) qiFlow(context, time);
            if (windows.outcome().sessionId() > 0) outcome(context, time);
            var renderer = MinecraftClient.getInstance().textRenderer;
            String caption = windows.outcome().sessionId() > 0 ? resultLabel(phase) : windows.model().stepLabel();
            context.drawText(renderer, caption, x + 12, y + 12, accent(), false);
            String hint = switch (phase) {
                case "prepare" -> "将背包材料拖入下方炉口";
                case "billet" -> "坯料已入炉 · 材料不可取回";
                case "tempering" -> "轻击 J · 重击 K · 折叠 L";
                case "inscription" -> "将铭文残卷拖入刻纹区域";
                case "consecration" -> "按住注入 · 松开停止";
                default -> "拖动查看 · 滚轮缩放";
            };
            context.drawText(renderer, renderer.trimToWidth(hint, Math.max(1, width - 24)),
                x + 12, y + height - 18, 0xFFCEBA9E, false);
        } finally {
            context.disableScissor();
        }
    }

    private void embers(OwoUIDrawContext context, float time) {
        double impact = Math.max(0, 1 - (System.nanoTime() - lastImpact) / 650_000_000.0);
        for (int index = 0; index < 18; index++) {
            float progress = (time * .21f + index * .137f) % 1;
            int px = x + width / 2 + (int) (Math.sin(index * 8.3) * width * (.14 + impact * .22));
            int py = y + height - 25 - (int) (progress * height * .63);
            int alpha = (int) ((1 - progress) * (90 + impact * 150));
            context.fill(px, py, px + 1, py + 2, alpha << 24 | 0xF7A453);
        }
    }

    private void inscription(OwoUIDrawContext context, float time) {
        var state = InscriptionPanelComponent.renderStateFrom(windows.model().session());
        int slots = Math.max(1, state.maxSlots());
        for (int index = 0; index < slots; index++) {
            double angle = index * Math.PI * 2 / slots + time * .12;
            int cx = x + width / 2 + (int) (Math.cos(angle) * width * .3);
            int cy = y + height / 2 + (int) (Math.sin(angle) * height * .28);
            int color = index < state.filledCount() ? 0xFFD4BDF5 : 0x665E536A;
            context.drawRectOutline(cx - 7, cy - 7, 14, 14, color);
            context.fill(cx - 3, cy - 1, cx + 4, cy + 1, color);
            context.fill(cx - 1, cy - 4, cx + 1, cy + 5, color);
        }
    }

    private void qiFlow(OwoUIDrawContext context, float time) {
        for (int arm = 0; arm < 4; arm++) {
            for (int point = 0; point < 20; point++) {
                double progress = (time * .32 + point / 20.0) % 1;
                double angle = arm * Math.PI / 2 + progress * 2.5;
                int cx = x + width / 2 + (int) (Math.cos(angle) * width * .43 * (1 - progress));
                int cy = y + height / 2 + (int) (Math.sin(angle) * height * .37 * (1 - progress));
                int alpha = (int) (170 * Math.sin(progress * Math.PI));
                context.fill(cx, cy, cx + 2, cy + 2, alpha << 24 | 0x8ED9C1);
            }
        }
    }

    private void outcome(OwoUIDrawContext context, float time) {
        if (time > 2.8f) return;
        int alpha = Math.max(0, (int) (80 * (1 - time / 2.8f)));
        if (phase.equals("perfect") || phase.equals("good")) {
            int center = x + (int) (width * Math.min(1, time / 1.4));
            context.fillGradient(center - 10, y + 2, center + 10, y + height - 2, alpha << 24 | (accent() & 0xFFFFFF), 0);
        } else if (phase.equals("explode")) {
            context.fill(x, y, x + width, y + height, alpha << 24 | 0xB54628);
        }
    }

    public boolean furnaceAt(double mouseX, double mouseY) {
        return isInBoundingBox(mouseX, mouseY) && mouseY >= y + height * .68;
    }

    @Override public boolean onMouseScroll(double mouseX, double mouseY, double amount) {
        // owo 的 ScrollContainer 先把滚轮转给子节点；右侧留出通道让窄窗仍可滚动正文。
        if (mouseX >= width - 6) return false;
        return super.onMouseScroll(mouseX, mouseY, amount);
    }

    private float seconds() { return (System.nanoTime() - phaseStarted) / 1_000_000_000f; }
    private int accent() {
        return switch (phase) {
            case "perfect" -> 0xFFFFD892;
            case "good", "consecration" -> 0xFF9ED8C1;
            case "inscription" -> 0xFFD4BDF5;
            case "flawed", "waste", "explode" -> 0xFFCC876B;
            default -> 0xFFF0B77C;
        };
    }

    public static String resultLabel(String bucket) {
        return switch (bucket) {
            case "perfect" -> "精良 · 火候尽得";
            case "good" -> "良品 · 器已成形";
            case "flawed" -> "瑕品 · 留有缺憾";
            case "explode" -> "炸炉 · 炉火失控";
            default -> "废品 · 此炉未成";
        };
    }

    private static void sound(net.minecraft.sound.SoundEvent event, float volume, float pitch) {
        var client = MinecraftClient.getInstance();
        if (client.player != null) client.player.playSound(event, volume, pitch);
    }
}
