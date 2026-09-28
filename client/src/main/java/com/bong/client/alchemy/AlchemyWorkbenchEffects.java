package com.bong.client.alchemy;

import com.bong.client.alchemy.AlchemyFurnaceComponent.Artwork;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.inspect.ItemInspectModel;
import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.model.InventoryItem;
import com.mojang.blaze3d.systems.RenderSystem;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.util.ScissorStack;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.model.ModelLoader;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.RotationAxis;

/**
 * 工位中的视觉效果，不处理输入、不发送请求。
 * 只有服务器确认的投料、注元和收取事件才启动瞬时动画。
 */
final class AlchemyWorkbenchEffects {
    private final AlchemyWindows windows;
    private long effectRevision;
    private long resultRevision;
    private long resultStart;
    private AlchemyResultEffect resultEffect;
    private long smokeStart;
    private long qiStart;
    private int smokeColor = 0x9DAF9E;
    private InventoryItem feedingItem;
    private ItemInspectModel feedingModel;
    private InventoryItem resultItem;
    private ItemInspectModel resultModel;
    private String resultName = "炼制残渣";
    private static final Identifier[] SMOKE = java.util.stream.IntStream.range(0, 12)
        .mapToObj(index -> new Identifier("minecraft", "textures/particle/big_smoke_" + index + ".png"))
        .toArray(Identifier[]::new);

    AlchemyWorkbenchEffects(AlchemyWindows windows) {
        this.windows = windows;
        var previous = windows.confirmedEffect();
        effectRevision = previous == null ? 0 : previous.revision();
        var previousResult = windows.confirmedResult();
        resultRevision = previousResult == null ? 0 : previousResult.revision();
    }

    void tick(BongModeledEntity preview) {
        if (preview != null) {
            preview.setAlchemyHeat(windows.active() ? (float) windows.model().session().tempCurrent() : 0);
            preview.setVisualState(windows.active() ? 1 : 0);
            preview.tick();
        }
        var result = windows.confirmedResult();
        if (result != null && result.revision() != resultRevision) {
            resultRevision = result.revision();
            resultEffect = result.effect();
            resultStart = System.nanoTime();
            resultItem = result.item();
            resultName = result.displayName();
            resultModel = resultItem == null ? null : ItemInspectModel.find(resultItem.itemId()).orElse(null);
            if (preview != null) preview.playAlchemyResult(resultEffect);
        }
        var effect = windows.confirmedEffect();
        if (effect == null || effect.revision() == effectRevision) return;
        effectRevision = effect.revision();
        if (effect.qi()) qiStart = System.nanoTime();
        else if (!effect.material().equals("incense")) {
            if (preview != null) preview.playAlchemyFeed();
            smokeColor = AlchemyMaterialColors.color(effect.material());
            smokeStart = System.nanoTime();
            feedingItem = effect.item();
            feedingModel = feedingItem == null ? null : ItemInspectModel.find(feedingItem.itemId()).orElse(null);
        }
    }


    /** 阴影、火焰和香座先画，炉体随后盖住火焰，让炉脚有真实遮挡关系。 */
    void drawBehindFurnace(OwoUIDrawContext context, Artwork art) {
        drawGrounding(context, art);
        drawHearth(context, art);
    }

    /** 投料和结算置于前景；投料下降时仍须由炉口边界裁去下缘。 */
    void drawInFrontOfFurnace(OwoUIDrawContext context, Artwork art) {
        drawResult(context, art);
        drawFeed(context, art);
    }

    private void drawGrounding(OwoUIDrawContext context, Artwork art) {
        int center = art.x() + (int) (art.width() * .56);
        int floor = art.y() + (int) (art.height() * .83);
        ellipse(context, center, floor, (int) (art.width() * .17), Math.max(3, art.height() / 35), 0x3F0B100D);
        ellipse(context, center, floor, (int) (art.width() * .12), Math.max(2, art.height() / 55), 0x550B100D);
    }

    private static void ellipse(OwoUIDrawContext context, int centerX, int centerY, int radiusX, int radiusY, int color) {
        for (int offset = -radiusY; offset <= radiusY; offset++) {
            int half = (int) Math.round(radiusX * Math.sqrt(1 - (double) offset * offset / (radiusY * radiusY)));
            context.fill(centerX - half, centerY + offset, centerX + half + 1, centerY + offset + 1, color);
        }
    }

    private void drawHearth(OwoUIDrawContext context, Artwork art) {
        int center = art.x() + (int) (art.width() * .56);
        int floor = art.y() + (int) (art.height() * .79);
        int radius = Math.max(8, art.width() / 9);
        double now = System.nanoTime() / 1_000_000_000.0;
        if (windows.active()) {
            double heat = Math.max(0, Math.min(1, windows.model().session().tempCurrent()));
            if (heat > .01) {
                int flameHeight = Math.max(8, (int) (art.height() * (.06 + heat * .17)));
                context.fillGradient(center - radius, floor - flameHeight, center + radius, floor + 5,
                    0, ((int) (heat * 110)) << 24 | 0xE96921);
                var fire = ModelLoader.FIRE_0.getSprite();
                for (int index = 0; index < 3; index++) {
                    int flameWidth = Math.max(8, radius * 2 / 3);
                    int rise = (int) (flameHeight * (index == 1 ? 1 : .65) * (.85 + .15 * Math.sin(now * 7 + index * 2)));
                    int px = center - radius / 2 + index * radius / 3 - flameWidth / 2;
                    context.drawSprite(px, floor - rise, 0, flameWidth, rise, fire);
                }
                drawSmoke(context, art, now, heat, smokeStart == 0 ? 0xAAA69A : smokeColor);
            }
        }
        double smokeAge = now - smokeStart / 1_000_000_000.0;
        if (smokeStart != 0 && smokeAge < 2.8) {
            for (int index = 0; index < 18; index++) {
                double age = smokeAge - index * .055;
                if (age < 0 || age > 1.8) continue;
                int px = center + (int) (Math.sin(age * 3 + index) * 7);
                int py = floor - art.height() / 3 - (int) (age * 23);
                int size = Math.max(5, (int) (art.width() * .018 * (1 + age)));
                smokePuff(context, px, py, size, age / 1.8, smokeColor, .45f);
            }
        }
        double qiAge = now - qiStart / 1_000_000_000.0;
        if (qiStart != 0 && qiAge < 1.2) {
            for (int index = 0; index < 20; index++) {
                double angle = index * .5 + qiAge * 5;
                int px = center + (int) (Math.cos(angle) * radius * (1 - qiAge / 1.2));
                int py = floor - art.height() / 4 + (int) (Math.sin(angle) * 12);
                context.fill(px, py, px + 2, py + 2, ((int) (170 * (1 - qiAge / 1.2))) << 24 | 0x97D6BE);
            }
        }
        drawIncense(context, art, now);
    }

    private void drawIncense(OwoUIDrawContext context, Artwork art, double now) {
        var incense = windows.incenseView();
        int baseX = art.x() + (int) (art.width() * .18);
        int baseY = art.y() + (int) (art.height() * .93);
        int bowl = Math.max(6, art.width() / 35);
        ellipse(context, baseX, baseY + 2, bowl, 3, 0xFF4B3930);
        ellipse(context, baseX, baseY, bowl, 2, 0xFF98724C);
        ellipse(context, baseX, baseY - 1, bowl - 2, 1, 0xFF3A342A);
        if (incense.state() == AlchemyIncenseTimer.State.EMPTY) return;
        int tipY = baseY - (int) (art.height() * .11 * incense.remainingFraction());
        context.fill(baseX, tipY, baseX + 2, baseY, 0xFF8B684A);
        if (incense.state() != AlchemyIncenseTimer.State.BURNING) return;
        context.fill(baseX, tipY - 1, baseX + 2, tipY + 1, 0xFFF4A65D);
        int color = Integer.parseInt(incense.smokeColor().substring(1), 16);
        for (int index = 0; index < 18; index++) {
            double age = (now * .35 + index / 18.0) % 1;
            int px = baseX + (int) (Math.sin(age * 6 + now) * 5);
            int py = tipY - 3 - (int) (age * art.height() * .18);
            smokePuff(context, px, py, Math.max(3, (int) (art.width() * .012)), age, color, .4f);
        }
    }

    private void drawSmoke(OwoUIDrawContext context, Artwork art, double now, double heat, int color) {
        int count = 8 + (int) (heat * 20);
        for (int index = 0; index < count; index++) {
            double age = (now * (.22 + heat * .2) + index / (double) count) % 1;
            int px = art.x() + (int) (art.width() * (.56 + Math.sin(age * 4 + index) * .025 * age));
            int py = art.y() + (int) (art.height() * (.20 - age * (.14 + heat * .1)));
            int size = Math.max(6, (int) (art.width() * (.025 + age * .035) * (.6 + heat)));
            smokePuff(context, px, py, size, age, color, (float) (.25 + heat * .3));
        }
    }

    private static void smokePuff(OwoUIDrawContext context, int px, int py, int size,
                                   double age, int color, float opacity) {
        context.draw();
        float[] previous = RenderSystem.getShaderColor().clone();
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        RenderSystem.setShaderColor((color >> 16 & 255) / 255f, (color >> 8 & 255) / 255f,
            (color & 255) / 255f, (float) (opacity * Math.sin(Math.PI * age)));
        try {
            context.drawTexture(SMOKE[Math.min(11, (int) (age * 12))], px - size / 2, py - size / 2,
                size, size, 0, 0, 16, 16, 16, 16);
            context.draw();
        } finally {
            RenderSystem.setShaderColor(previous[0], previous[1], previous[2], previous[3]);
        }
    }

    private void drawFeed(OwoUIDrawContext context, Artwork art) {
        if (feedingItem == null) return;
        double age = (System.nanoTime() - smokeStart) / 1_000_000_000.0;
        if (age > .8) return;
        double approach = Math.min(1, age / .38);
        double descent = Math.max(0, (age - .38) / .42);
        int px = art.x() + (int) (art.width() * (.38 + .18 * approach));
        int py = art.y() + (int) (art.height() * (.35 - .12 * Math.sin(approach * Math.PI / 2) + .23 * descent));
        int size = Math.max(4, (int) (art.width() * .065 * (1 - descent * .55)));
        // 下沉阶段由炉口遮住物品下缘，避免图标透过炉身。
        ScissorStack.push(art.x(), art.y(), art.width(), (int) (art.height() * .37), context.getMatrices());
        try {
            if (feedingModel == null) {
                GridSlotComponent.drawItemTexture(context, feedingItem, px - size / 2, py - size / 2, size, size);
            } else {
                var bounds = feedingModel.bounds();
                var center = bounds.getCenter();
                float scale = (float) (size / Math.max(.01, Math.sqrt(bounds.getXLength() * bounds.getXLength()
                    + bounds.getYLength() * bounds.getYLength() + bounds.getZLength() * bounds.getZLength())));
                var matrices = context.getMatrices();
                var buffers = MinecraftClient.getInstance().getBufferBuilders().getEntityVertexConsumers();
                context.draw();
                matrices.push();
                try {
                    matrices.translate(px, py, 30);
                    matrices.scale(scale, feedingModel.yUp() ? -scale : scale, scale * .1f);
                    matrices.multiply(RotationAxis.POSITIVE_Y.rotationDegrees((float) (age * 150)));
                    matrices.translate(-center.x, -center.y, -center.z);
                    RenderSystem.enableDepthTest();
                    feedingModel.render(matrices, buffers);
                    buffers.draw();
                } finally {
                    matrices.pop();
                    RenderSystem.disableDepthTest();
                }
            }
        } finally {
            context.draw();
            ScissorStack.pop();
        }
    }

    private void drawResult(OwoUIDrawContext context, Artwork art) {
        if (resultEffect == null) return;
        double age = (System.nanoTime() - resultStart) / 1_000_000_000.0;
        double duration = resultEffect.ticks() / 20.0;
        // 旋转一圈后停留片刻，让玩家看清成品名称；收取展示不应刚升起就消失。
        double hold = 1.4;
        if (age >= duration + hold) return;
        double progress = Math.min(1.0, age / duration);
        boolean explode = resultEffect == AlchemyResultEffect.EXPLODE;
        int color = switch (resultEffect) {
            case EXPLODE -> 0xF7A15A;
            case WASTE -> 0x615953;
            case FLAWED -> 0x9B9876;
            case EARLY_TAKE -> 0xD4D1BD;
            case COMPLETE -> 0xD4B66F;
        };
        int center = art.x() + (int) (art.width() * .56);
        int mouth = art.y() + (int) (art.height() * .32);
        if (explode && progress < .16) {
            context.fill(art.x(), art.y(), art.x() + art.width(), art.y() + art.height(),
                (int) (70 * (1 - progress / .16)) << 24 | 0xE39154);
        }
        int count = explode ? 36 : 18;
        for (int index = 0; index < count; index++) {
            double angle = index * 2.4;
            double spread = art.width() * (explode ? .28 : .065) * progress;
            int px = center + (int) (Math.cos(angle) * spread);
            int py = mouth + (int) (Math.sin(angle) * spread * .55
                - art.height() * .16 * progress + (explode ? art.height() * .16 * progress * progress : 0));
            int size = Math.max(1, (int) (art.width() * (explode ? .006 : .012) * (1 + progress)));
            int alpha = (int) (190 * (1 - progress));
            context.fill(px, py, px + size, py + size, alpha << 24 | color);
        }
        // 炸炉也有可回收的炉渣；爆闪结束后照样展示残渣名称，避免只剩粒子没有结果。
        if (progress > (explode ? .34 : .12)) drawResultItem(context, art, progress);
    }

    private void drawResultItem(OwoUIDrawContext context, Artwork art, double progress) {
        if (resultItem == null) return;
        double rise = Math.min(1.0, Math.max(0.0, (progress - .12) / .58));
        double eased = rise * rise * (3.0 - 2.0 * rise);
        int center = art.x() + (int) (art.width() * .56);
        int mouth = art.y() + (int) (art.height() * .34);
        int px = center;
        int py = mouth - (int) (art.height() * (.06 + eased * .22));
        int size = Math.max(12, (int) (art.width() * .12 * (0.72 + eased * .28)));
        var matrices = context.getMatrices();
        matrices.push();
        try {
            matrices.translate(px, py, 42);
            // 平面图标绕屏幕法线旋转；有真实模型时绕竖直轴展示一整圈。
            matrices.multiply((resultModel == null ? RotationAxis.POSITIVE_Z : RotationAxis.POSITIVE_Y)
                .rotationDegrees((float) (progress * 360.0)));
            if (resultModel == null) {
                GridSlotComponent.drawItemTexture(context, resultItem, -size / 2, -size / 2, size, size);
            } else {
                var bounds = resultModel.bounds();
                var centerPoint = bounds.getCenter();
                float scale = (float) (size / Math.max(.01, Math.sqrt(
                    bounds.getXLength() * bounds.getXLength()
                        + bounds.getYLength() * bounds.getYLength()
                        + bounds.getZLength() * bounds.getZLength())));
                var buffers = MinecraftClient.getInstance().getBufferBuilders().getEntityVertexConsumers();
                context.draw();
                matrices.push();
                matrices.scale(scale, resultModel.yUp() ? -scale : scale, scale * .1f);
                matrices.translate(-centerPoint.x, -centerPoint.y, -centerPoint.z);
                RenderSystem.enableDepthTest();
                resultModel.render(matrices, buffers);
                buffers.draw();
                RenderSystem.disableDepthTest();
                matrices.pop();
            }
        } finally {
            context.draw();
            matrices.pop();
        }
        var text = MinecraftClient.getInstance().textRenderer;
        int labelWidth = text.getWidth(resultName);
        int labelX = center - labelWidth / 2;
        int labelY = py + size / 2 + 5;
        context.fill(labelX - 5, labelY - 3, labelX + labelWidth + 5, labelY + text.fontHeight + 2, 0xB9151B17);
        context.drawTextWithShadow(text, net.minecraft.text.Text.literal(resultName), labelX, labelY, 0xFFF3D59B);
    }

}
