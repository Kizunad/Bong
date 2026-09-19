package com.bong.client.inspect;

import com.bong.client.artifact.ArtifactState;
import com.bong.client.inventory.RarityVisuals;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.ui.adapter.owo.OwoXmlTemplateRegistry;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/** 物品详情只呈现服务器元数据，XML 负责属性列与长描述的布局。 */
public final class ItemInspectContent {
    public record Detail(String label, String value) {}

    private ItemInspectContent() {}

    public static void bind(FlowLayout root, InventoryItem item) {
        var template = OwoXmlTemplateRegistry.production().require("item-inspect");
        FlowLayout iconSlot = root.childById(FlowLayout.class, "item-icon");
        var media = iconSlot.childById(ItemModelPreviewComponent.class, "item-media");
        if (media == null || !media.itemId().equals(item.itemId())) {
            for (var child : List.copyOf(iconSlot.children())) iconSlot.removeChild(child);
            var preview = new ItemModelPreviewComponent(item.itemId());
            preview.horizontalSizing(iconSlot.horizontalSizing().get());
            if (preview.hasModel()) {
                var toggle = template.expandTemplate(ButtonComponent.class, "view-toggle", Map.of());
                toggle.onPress(button -> {
                    preview.toggleModel();
                    button.setMessage(Text.literal(preview.showingModel() ? "PNG" : "3D"));
                });
                iconSlot.child(toggle);
            }
            iconSlot.child(preview);
        }
        FlowLayout details = root.childById(FlowLayout.class, "item-details");
        for (var child : List.copyOf(details.children())) details.removeChild(child);
        for (Detail detail : detailRows(item)) {
            var row = template.expandTemplate(FlowLayout.class, "detail-row", Map.of());
            row.childById(LabelComponent.class, "detail-label").text(Text.literal(detail.label() + ":"));
            row.childById(LabelComponent.class, "detail-value").text(Text.literal(detail.value()));
            row.surface((context, component) -> context.fill(component.x(), component.y() + component.height() - 1,
                component.x() + component.width(), component.y() + component.height(), 0xFF353B37));
            details.child(row);
        }
        FlowLayout notes = root.childById(FlowLayout.class, "item-notes");
        for (var child : List.copyOf(notes.children())) notes.removeChild(child);
        List<String> paragraphs = new ArrayList<>();
        if (!item.description().isBlank()) paragraphs.add(item.description());
        paragraphs.addAll(item.alchemyLines());
        for (String paragraph : paragraphs) {
            var label = template.expandTemplate(LabelComponent.class, "paragraph", Map.of());
            label.text(Text.literal(paragraph));
            notes.child(label);
        }
    }

    public static List<Detail> detailRows(InventoryItem item) {
        if (item == null || item.isEmpty()) return List.of();
        List<Detail> rows = new ArrayList<>();
        rows.add(new Detail("名称", item.displayName()));
        rows.add(new Detail("稀有度", RarityVisuals.label(item.rarity())));
        if (item.spiritQuality() > 0) rows.add(new Detail("灵质", percent(item.spiritQuality())));
        rows.add(new Detail("重量", String.format(Locale.ROOT, "%.1f kg", item.weight())));
        rows.add(new Detail("占格", item.gridWidth() + " x " + item.gridHeight()));
        if (item.stackCount() > 1) rows.add(new Detail("数量", Integer.toString(item.stackCount())));
        // durability 是损耗比例；保质期由独立 freshness 元数据表达，不能互用。
        if (item.durability() < 1) rows.add(new Detail("耐久", percent(item.durability())));
        if (item.charges() != null) rows.add(new Detail("充能次数", Integer.toString(item.charges())));
        if (item.forgeQuality() != null) rows.add(new Detail("炼成品质", percent(item.forgeQuality())));
        if (item.forgeAchievedTier() != null) rows.add(new Detail("灵核", "T" + item.forgeAchievedTier()));
        if (item.hasInscription()) rows.add(new Detail("铭文", item.inscriptionId()));
        var effects = item.visibleForgeSideEffects();
        if (!effects.isEmpty()) rows.add(new Detail("当前附着", String.join(", ", effects)));
        item.artifactState().ifPresent(artifact -> appendArtifactRows(rows, artifact));
        return List.copyOf(rows);
    }

    private static String percent(double ratio) {
        return Math.round(Math.max(0.0, Math.min(1.0, ratio)) * 100.0) + "%";
    }

    private static void appendArtifactRows(List<Detail> rows, ArtifactState artifact) {
        rows.add(new Detail("铭纹", artifact.grooveCount() + "槽"));
        rows.add(new Detail("铭纹深度", String.format(Locale.ROOT, "%.1f / %.0f", artifact.averageDepth(), artifact.depthCap())));
        rows.add(new Detail("共鸣提示", percent(artifact.resonancePreview())));
        rows.add(new Detail("器色", artifact.mainColorLabel()));
        rows.add(new Detail("龟裂", artifact.crackLabel()));
        if (artifact.overloadCracks() > 0) rows.add(new Detail("过载裂纹", Integer.toString(artifact.overloadCracks())));
    }
}
