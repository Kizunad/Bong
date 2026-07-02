package com.bong.client.inventory.component;

import com.bong.client.armor.ArmorTintRegistry;
import com.bong.client.botany.BotanySpiritQualityVisuals;
import com.bong.client.inventory.RarityVisuals;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.SlotContents;
import com.bong.client.inventory.AncientRelicGlowRenderer;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.text.Text;

import java.util.Locale;

public class ItemTooltipPanel extends BaseComponent {
    private static final int PANEL_WIDTH = 196;
    /**
     * 空面板/hint 默认高度，也是最小高度保证 icon 高度不被裁 + 常见 description 完整显示。
     * 统计当前所有 item description 最长 92 字符（≈ 46 汉字 ≈ 4 行全宽），top
     * 固定（padding + name + meta + optional status）最大 37 px，加 padding_bottom
     * 和 desc 行高估算约 81 px。112 给足余量；超出的长描述仍由动态 sizing 自动扩展。
     */
    private static final int DEFAULT_HEIGHT = 112;
    // plan-inventory-hint-panel-v1 §P3 视听规格：hover 面板背景 0xE0141414（较原
    // 0xCC181818 更深更不透明，与失败 toast 的警示语感区分开——toast 走瞬时红色文字，
    // hover 面板走常驻深色底板 + 逐行红/绿约束提示）。
    private static final int BG_COLOR = 0xE0141414;
    private static final int BORDER_COLOR = 0xFF3A3A3A;
    private static final int HINT_COLOR = 0x60AAAAAA;

    // plan-inventory-hint-panel-v1 §P3：hover 目标切换时面板淡入 4 tick（20 tick/s ⇒ 200ms）。
    // 只淡入面板"底板+边框"（chrome），不淡入文字——文字读数优先即时可读，边框/底色的渐显
    // 只为视觉过渡不生硬，不影响信息可读性。
    static final long FADE_DURATION_MILLIS = 200L;

    // Icon 占左上角一个正方形，文字从 icon 右边起。
    private static final int ICON_SIZE = 32;
    private static final int ICON_MARGIN = 4;
    private static final int TEXT_LEFT_OFFSET = ICON_MARGIN + ICON_SIZE + 4;
    private static final int PADDING_TOP = 4;
    private static final int PADDING_BOTTOM = 4;
    private static final int DESC_LINE_STEP = 1;
    private static final int BLOCK_LINE_STEP = 2;
    private static final int QUALITY_BAR_HEIGHT = 3;
    private static final int QUALITY_TRACK_COLOR = 0x66000000;

    // plan-inventory-hint-panel-v1 P2「约束说明」区配色（与 §P3 视听规格一致：满足=绿/不满足=红）。
    static final int CONSTRAINT_OK_COLOR = 0xFF66BB66;
    static final int CONSTRAINT_FULL_COLOR = 0xFFCC5555;
    static final int CONSTRAINT_NEUTRAL_COLOR = 0xFFAAAAAA;
    private static final int EMPTY_SLOT_HEADER_COLOR = 0xFFCCCCCC;

    private InventoryItem hoveredItem;
    private EquipSlotType hoveredSlotType;
    private int hoveredSlotWornCount;
    private int currentHeight = DEFAULT_HEIGHT;
    // plan-inventory-hint-panel-v1 §P3：hover 目标（item + slotType 组合）上次变化的时间戳，
    // 驱动面板 chrome 4-tick 淡入。初值 0 使首次 draw() 即 elapsed 极大 ⇒ fadeProgress=1（不留白）。
    private long hoverChangedAtMillis;

    public ItemTooltipPanel() {
        this.sizing(Sizing.fixed(PANEL_WIDTH), Sizing.fixed(DEFAULT_HEIGHT));
    }

    public void setHoveredItem(InventoryItem item) {
        applyHover(item, null, 0);
    }

    /**
     * plan-inventory-hint-panel-v1 P2：装备槽 hover 专属入口——额外传入槽身份 + worn 层数，
     * 使面板即便槽为空（{@code contents.representative()==null}）也能显示"约束说明"区
     * （核心价值：预看 {@code worn_cap}，不必等失败 toast 才知道，plan §P2）。
     */
    public void setHoveredEquipSlot(EquipSlotType slotType, SlotContents contents) {
        InventoryItem item = contents == null ? null : contents.representative();
        int wornCount = contents == null ? 0 : contents.wornCount();
        applyHover(item, slotType, wornCount);
    }

    private void applyHover(InventoryItem item, EquipSlotType slotType, int wornCount) {
        if (targetChanged(hoveredItem, hoveredSlotType, item, slotType)) {
            hoverChangedAtMillis = System.currentTimeMillis();
        }
        this.hoveredItem = item;
        this.hoveredSlotType = slotType;
        this.hoveredSlotWornCount = wornCount;
        int required = computeRequiredHeight(item, slotType);
        if (required != currentHeight) {
            currentHeight = required;
            // owo-lib BaseComponent.sizing 是 Observable，改值会自动触发 notifyParentIfMounted，
            // parent FlowLayout 随之重新 inflate，新高度本轮或下一轮渲染即生效。
            this.sizing(Sizing.fixed(PANEL_WIDTH), Sizing.fixed(currentHeight));
        }
    }

    private int computeRequiredHeight(InventoryItem item, EquipSlotType slotType) {
        // 空槽但来自装备槽 hover（slotType != null）：仍需渲染"约束说明"区（非通用 hint），
        // DEFAULT_HEIGHT 足够容纳槽名 + 约束行两行，无需额外扩高。
        if (item == null || item.isEmpty()) return DEFAULT_HEIGHT;

        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;
        int lineBlock = textRenderer.fontHeight + BLOCK_LINE_STEP;

        // 顶部固定：padding + name + meta +（可选）status
        int needed = PADDING_TOP + lineBlock + lineBlock + lineBlock + QUALITY_BAR_HEIGHT + 2;
        if (!formatStatusLine(item).isEmpty()) {
            needed += lineBlock;
        }
        if (BotanySpiritQualityVisuals.isBotanyPlant(item)) {
            needed += lineBlock + 6;
        }
        if (item.forgeQuality() != null) {
            needed += lineBlock;
        }
        if (!item.visibleForgeSideEffects().isEmpty()) {
            needed += lineBlock;
        }
        if (!item.alchemyLines().isEmpty()) {
            needed += lineBlock * item.alchemyLines().size();
        }

        // plan-armor-v1 §5：护甲矩阵（仅护甲类物品显示）。
        if (com.bong.client.combat.ArmorProfileStore.isArmor(item.itemId())) {
            needed += lineBlock * 2;
        }
        if (ArmorTintRegistry.isMundaneArmor(item.itemId())) {
            needed += lineBlock * (item.durability() <= 0.0 ? 4 : 3);
        }

        // top 部分至少保证 icon 高度（描述推到 icon 底部之下显示）。
        needed = Math.max(needed, ICON_MARGIN + ICON_SIZE);

        // description 用 TextRenderer.wrapLines 做真正的 word-wrap，按全宽计算。
        if (!item.description().isEmpty()) {
            int maxWidth = PANEL_WIDTH - ICON_MARGIN * 2;
            int lines = textRenderer.wrapLines(Text.literal(item.description()), maxWidth).size();
            needed += lines * (textRenderer.fontHeight + DESC_LINE_STEP);
        }
        // plan-inventory-hint-panel-v1 P2：装备槽 hover 时（slotType != null）在描述后追加一行"约束说明"。
        if (slotType != null) {
            needed += lineBlock;
        }
        needed += PADDING_BOTTOM;

        return Math.max(DEFAULT_HEIGHT, needed);
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        int h = this.height;
        float fade = fadeProgress(System.currentTimeMillis() - hoverChangedAtMillis);
        context.fill(x, y, x + PANEL_WIDTH, y + h, applyFadeAlpha(BG_COLOR, fade));
        GridSlotComponent.drawSlotBorder(context, x, y, PANEL_WIDTH, h, applyFadeAlpha(BORDER_COLOR, fade));
        if (AncientRelicGlowRenderer.shouldGlow(hoveredItem)) {
            AncientRelicGlowRenderer.drawGlowBorder(context, x, y, PANEL_WIDTH, h, System.currentTimeMillis());
        }

        TextRenderer textRenderer = MinecraftClient.getInstance().textRenderer;

        if (hoveredItem == null || hoveredItem.isEmpty()) {
            if (hoveredSlotType != null) {
                // plan-inventory-hint-panel-v1 P2：空装备槽 hover——核心价值恰在此处（cap 预看，
                // 不必等失败 toast 才知道）。不落回通用 "移动光标至物品查看详情" hint。
                drawEmptySlotConstraint(context, textRenderer);
                return;
            }
            String hint = "移动光标至物品查看详情";
            int hintX = x + (PANEL_WIDTH - textRenderer.getWidth(hint)) / 2;
            int hintY = y + (h - textRenderer.fontHeight) / 2;
            context.drawTextWithShadow(textRenderer, Text.literal(hint), hintX, hintY, HINT_COLOR);
            return;
        }

        // 左上角 icon —— 复用 GridSlotComponent.drawItemTexture（含内部 z=100 push + blend 设置）。
        GridSlotComponent.drawItemTexture(
            context, hoveredItem,
            x + ICON_MARGIN, y + ICON_MARGIN,
            ICON_SIZE, ICON_SIZE
        );

        int cy = y + PADDING_TOP;
        int cx = x + TEXT_LEFT_OFFSET;
        int descLeft = x + ICON_MARGIN;

        // Item name with rarity color
        context.drawTextWithShadow(textRenderer,
            Text.literal(hoveredItem.displayName()),
            cx, cy, nameColor(hoveredItem));
        cy += textRenderer.fontHeight + BLOCK_LINE_STEP;

        // Rarity + size
        String meta = rarityLabel(hoveredItem.rarity())
            + " | " + hoveredItem.gridWidth() + "×" + hoveredItem.gridHeight()
            + " | " + String.format(Locale.ROOT, "%.1f", hoveredItem.weight()) + "kg";
        if (hoveredItem.stackCount() > 1) {
            meta += " | x" + hoveredItem.stackCount();
        }
        context.drawTextWithShadow(textRenderer, Text.literal(meta), cx, cy, 0xFF888888);
        cy += textRenderer.fontHeight + BLOCK_LINE_STEP;

        context.drawTextWithShadow(textRenderer, Text.literal(spiritQualityLabel(hoveredItem)), cx, cy, qualityBarColor(hoveredItem.spiritQuality()));
        cy += textRenderer.fontHeight + 1;
        drawSpiritQualityBar(context, x + ICON_MARGIN, cy, PANEL_WIDTH - ICON_MARGIN * 2, hoveredItem.spiritQuality());
        cy += QUALITY_BAR_HEIGHT + BLOCK_LINE_STEP;

        // 真元 / 耐久 —— 仅当 < 1.0 时显示，避免新玩家信息过载。
        String status = formatStatusLine(hoveredItem);
        if (!status.isEmpty()) {
            context.drawTextWithShadow(textRenderer, Text.literal(status), cx, cy, statusColor(hoveredItem));
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        if (BotanySpiritQualityVisuals.isBotanyPlant(hoveredItem)) {
            String qualityLabel = BotanySpiritQualityVisuals.qualityLabel(hoveredItem);
            context.drawTextWithShadow(textRenderer, Text.literal(qualityLabel), cx, cy, BotanySpiritQualityVisuals.barColor(hoveredItem));
            cy += textRenderer.fontHeight + 1;
            appendSpiritQualityBar(context, hoveredItem, cx, cy);
            cy += 5;
        }

        if (hoveredItem.forgeQuality() != null) {
            StringBuilder forge = new StringBuilder(String.format(
                Locale.ROOT,
                "炼成 %.0f%%",
                hoveredItem.forgeQuality() * 100
            ));
            if (hoveredItem.forgeAchievedTier() != null) {
                forge.append(" · ").append(hoveredItem.forgeAchievedTier()).append("阶");
            }
            if (!hoveredItem.forgeColor().isEmpty()) {
                forge.append(" · ").append(forgeColorLabel(hoveredItem.forgeColor()));
            }
            context.drawTextWithShadow(textRenderer, Text.literal(forge.toString()), cx, cy, 0xFF88DDBB);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        if (!hoveredItem.visibleForgeSideEffects().isEmpty()) {
            String sideEffects = "瑕疵 " + String.join("/", hoveredItem.visibleForgeSideEffects());
            context.drawTextWithShadow(textRenderer, Text.literal(sideEffects), cx, cy, 0xFFDDAA66);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        for (String line : hoveredItem.alchemyLines()) {
            context.drawTextWithShadow(textRenderer, Text.literal(line), cx, cy, 0xFFE0B060);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        if (ArmorTintRegistry.isMundaneArmor(hoveredItem.itemId())) {
            context.drawTextWithShadow(textRenderer, Text.literal(armorMaterialLine(hoveredItem)), cx, cy, 0xFF9A9A9A);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
            context.drawTextWithShadow(textRenderer, Text.literal(armorDefenseLine(hoveredItem)), cx, cy, 0xFF6FD080);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
            if (hoveredItem.durability() <= 0.0) {
                context.drawTextWithShadow(textRenderer, Text.literal(armorBrokenLine(hoveredItem)), cx, cy, 0xFFFF6666);
                cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
            }
            context.drawTextWithShadow(textRenderer, Text.literal(armorRepairLine(hoveredItem)), cx, cy, 0xFFAA8866);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        // plan-armor-v1 §5：护甲减免矩阵（WoundKind×系数）。
        com.bong.client.combat.ArmorProfileStore.ArmorMitigation mitigation =
            com.bong.client.combat.ArmorProfileStore.mitigationForItemId(hoveredItem.itemId());
        if (mitigation != null) {
            // Two compact rows: 斩/钝/刺 and 灼/震.
            String row1 = String.format(Locale.ROOT,
                "护甲 %s %s %s",
                renderMitigationCell("cut", mitigation.cut()),
                renderMitigationCell("blunt", mitigation.blunt()),
                renderMitigationCell("pierce", mitigation.pierce())
            );
            String row2 = String.format(Locale.ROOT,
                "     %s %s",
                renderMitigationCell("burn", mitigation.burn()),
                renderMitigationCell("concussion", mitigation.concussion())
            );

            context.drawTextWithShadow(textRenderer, Text.literal(row1), cx, cy, 0xFF88A0B0);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
            context.drawTextWithShadow(textRenderer, Text.literal(row2), cx, cy, 0xFF88A0B0);
            cy += textRenderer.fontHeight + BLOCK_LINE_STEP;
        }

        // Description —— 用 TextRenderer.wrapLines 做真正的 word-wrap（按字符宽度分行，不加 "…"）。
        // 为保证 wrap 宽度稳定，统一推到 icon 底部之下全宽显示，不再绕 icon 右侧。
        int iconBottom = y + ICON_MARGIN + ICON_SIZE;
        String desc = hoveredItem.description();
        if (!desc.isEmpty()) {
            cy = Math.max(cy, iconBottom);
            int maxWidth = PANEL_WIDTH - ICON_MARGIN * 2;
            for (var line : textRenderer.wrapLines(Text.literal(desc), maxWidth)) {
                if (cy > y + h - textRenderer.fontHeight - 2) break;
                context.drawTextWithShadow(textRenderer, line, descLeft, cy, 0xFFAAAAAA);
                cy += textRenderer.fontHeight + DESC_LINE_STEP;
            }
        }

        // plan-inventory-hint-panel-v1 P2：装备槽 hover（有件的槽）—— 追加一行"约束说明"。
        if (hoveredSlotType != null) {
            if (cy > y + h - textRenderer.fontHeight - PADDING_BOTTOM) {
                cy = y + h - textRenderer.fontHeight - PADDING_BOTTOM;
            }
            String constraintLine = slotConstraintLine(hoveredSlotType, hoveredSlotWornCount);
            int constraintColor = slotConstraintColor(hoveredSlotType, hoveredSlotWornCount);
            context.drawTextWithShadow(textRenderer, Text.literal(constraintLine), descLeft, cy, constraintColor);
        }
    }

    /** 空槽（{@code hoveredItem==null}）但来自装备槽 hover：只画槽名 + 约束行，不画物品面板。 */
    private void drawEmptySlotConstraint(OwoUIDrawContext context, TextRenderer textRenderer) {
        int lx = x + ICON_MARGIN;
        int ly = y + PADDING_TOP;
        context.drawTextWithShadow(
            textRenderer, Text.literal(hoveredSlotType.displayName() + "（空）"), lx, ly, EMPTY_SLOT_HEADER_COLOR
        );
        ly += textRenderer.fontHeight + BLOCK_LINE_STEP;
        String constraintLine = slotConstraintLine(hoveredSlotType, hoveredSlotWornCount);
        int constraintColor = slotConstraintColor(hoveredSlotType, hoveredSlotWornCount);
        context.drawTextWithShadow(textRenderer, Text.literal(constraintLine), lx, ly, constraintColor);
    }

    /**
     * plan-inventory-hint-panel-v1 P2：装备槽"约束说明"文案（package-private 供测试直接断言）。
     * 手槽（held-only，{@link EquipSlotType#isHand()}）无 worn cap 概念，恒返回持械位提示；
     * 身体槽按 {@link EquipSlotComponent#wornCap(EquipSlotType)} 静态常量算叠层进度。
     * 满员措辞与 P1 toast（{@code InventoryMoveRejectedHandler.capFullMessage}）语感一致——
     * 均含"已穿戴 N 层，无法再叠加"，且本行额外带"已满"字面量（plan §P2 测试声明点名）。
     */
    static String slotConstraintLine(EquipSlotType slotType, int wornCount) {
        if (slotType.isHand()) {
            return "持械位 · 仅可持 1 件";
        }
        int cap = EquipSlotComponent.wornCap(slotType);
        if (wornCount >= cap) {
            return "已满 · 已穿戴 " + cap + " 层，无法再叠加";
        }
        return "可叠 " + wornCount + "/" + cap + " 层";
    }

    /** 约束行配色：手槽中性灰；身体槽按是否已满走绿/红（§P3 视听规格）。 */
    static int slotConstraintColor(EquipSlotType slotType, int wornCount) {
        if (slotType.isHand()) {
            return CONSTRAINT_NEUTRAL_COLOR;
        }
        int cap = EquipSlotComponent.wornCap(slotType);
        return wornCount >= cap ? CONSTRAINT_FULL_COLOR : CONSTRAINT_OK_COLOR;
    }

    /**
     * plan-inventory-hint-panel-v1 §P3：hover 目标（item + slotType 组合）是否较上一帧发生变化，
     * 驱动面板 chrome 淡入计时器复位。纯函数（不摸 {@code System.currentTimeMillis()}），供测试
     * 直接断言：同一 item/slot 连续多帧 → false（计时器不复位，淡入不会卡在 0）；item 或 slot
     * 任一变化（含 null↔非 null）→ true。
     */
    static boolean targetChanged(InventoryItem prevItem, EquipSlotType prevSlot, InventoryItem nextItem, EquipSlotType nextSlot) {
        return !java.util.Objects.equals(prevItem, nextItem) || prevSlot != nextSlot;
    }

    /**
     * plan-inventory-hint-panel-v1 §P3："fade-in 4 tick"（20 tick/s ⇒ {@link #FADE_DURATION_MILLIS}
     * = 200ms）淡入进度纯函数。{@code elapsedMillis<=0} → 0（刚复位，尚未淡入）；
     * {@code elapsedMillis>=FADE_DURATION_MILLIS} → 1（淡入完成，稳态全不透明）；
     * 区间内线性插值。
     */
    static float fadeProgress(long elapsedMillis) {
        if (elapsedMillis <= 0L) {
            return 0f;
        }
        if (elapsedMillis >= FADE_DURATION_MILLIS) {
            return 1f;
        }
        return (float) elapsedMillis / (float) FADE_DURATION_MILLIS;
    }

    /**
     * plan-inventory-hint-panel-v1 §P3：把淡入进度应用到 ARGB 色值的 alpha 通道（RGB 不变）。
     * 纯函数供测试直接断言边界：progress=0 → alpha=0（全透明）；progress=1 → alpha=原值不变；
     * progress 越界（&lt;0 或 &gt;1）被 clamp，不产生越界 alpha。
     */
    static int applyFadeAlpha(int argbColor, float progress) {
        float clamped = Math.max(0f, Math.min(1f, progress));
        int baseAlpha = (argbColor >>> 24) & 0xFF;
        int fadedAlpha = Math.round(baseAlpha * clamped);
        return (fadedAlpha << 24) | (argbColor & 0x00FFFFFF);
    }

    static String rarityLabel(String rarity) {
        return RarityVisuals.label(rarity);
    }

    private static String forgeColorLabel(String color) {
        return switch (color) {
            case "Sharp" -> "锐";
            case "Heavy" -> "厚";
            case "Mellow" -> "醇";
            case "Solid" -> "实";
            case "Light" -> "轻";
            case "Intricate" -> "巧";
            case "Gentle" -> "柔";
            case "Insidious" -> "阴";
            case "Violent" -> "烈";
            case "Turbid" -> "浊";
            default -> color;
        };
    }

    public static String formatStatusLine(InventoryItem item) {
        if (item == null || item.isEmpty()) return "";

        StringBuilder status = new StringBuilder();
        if (item.spiritQuality() < 1.0) {
            String label = item.isBoneCoin() ? "封灵真元" : "纯度";
            status.append(String.format(Locale.ROOT, "%s %.0f%%", label, item.spiritQuality() * 100));
        }
        if (item.durability() < 1.0) {
            if (status.length() > 0) status.append("  ");
            status.append(String.format(Locale.ROOT, "耐久 %.0f%%", item.durability() * 100));
        }
        String charges = AncientRelicGlowRenderer.chargesLine(item);
        if (!charges.isEmpty()) {
            if (status.length() > 0) status.append("  ");
            status.append(charges);
        }
        return status.toString();
    }

    public static String armorMaterialLine(InventoryItem item) {
        if (item == null || item.isEmpty()) return "";
        return ArmorTintRegistry.materialLine(item.itemId());
    }

    public static String armorDefenseLine(InventoryItem item) {
        if (item == null || item.isEmpty()) return "";
        return ArmorTintRegistry.defenseLine(item.itemId());
    }

    public static String armorBrokenLine(InventoryItem item) {
        if (item == null || item.isEmpty() || !ArmorTintRegistry.isMundaneArmor(item.itemId())) return "";
        return item.durability() <= 0.0 ? "已损坏·不可穿戴" : "";
    }

    public static String armorRepairLine(InventoryItem item) {
        if (item == null || item.isEmpty()) return "";
        return ArmorTintRegistry.repairLine(item.itemId());
    }

    public static String spiritQualityLabel(InventoryItem item) {
        if (item == null || item.isEmpty()) return "灵质 0%";
        return String.format(Locale.ROOT, "灵质 %.0f%%", item.spiritQuality() * 100);
    }

    public static int qualityBarFillWidth(int totalWidth, double spiritQuality) {
        int safeWidth = Math.max(0, totalWidth);
        double clamped = Math.max(0.0, Math.min(1.0, spiritQuality));
        return (int) Math.round(safeWidth * clamped);
    }

    public static int qualityBarColor(double spiritQuality) {
        double clamped = Math.max(0.0, Math.min(1.0, spiritQuality));
        if (clamped < 0.5) {
            return lerpRgb(0x888888, 0x22CC22, clamped / 0.5);
        }
        return lerpRgb(0x22CC22, 0xFFAA00, (clamped - 0.5) / 0.5);
    }

    private static void drawSpiritQualityBar(OwoUIDrawContext context, int left, int top, int width, double spiritQuality) {
        context.fill(left, top, left + width, top + QUALITY_BAR_HEIGHT, QUALITY_TRACK_COLOR);
        int fillWidth = qualityBarFillWidth(width, spiritQuality);
        if (fillWidth > 0) {
            context.fill(left, top, left + fillWidth, top + QUALITY_BAR_HEIGHT, 0xFF000000 | qualityBarColor(spiritQuality));
        }
    }

    private static int statusColor(InventoryItem item) {
        return (item.spiritQuality() < 0.3 || item.durability() < 0.3)
            ? 0xFFFF6666 : 0xFFAA8866;
    }

    private static int nameColor(InventoryItem item) {
        return ArmorTintRegistry.argbForItemIdOrDefault(item.itemId(), item.rarityColor());
    }

    private static void appendSpiritQualityBar(OwoUIDrawContext context, InventoryItem item, int bx, int by) {
        int width = 58;
        int filled = Math.max(0, Math.min(width, (int) Math.round(width * item.spiritQuality())));
        context.fill(bx, by, bx + width, by + 3, 0xFF222222);
        if (filled > 0) {
            context.fill(bx, by, bx + filled, by + 3, BotanySpiritQualityVisuals.barColor(item));
        }
        context.fill(bx, by + 3, bx + width, by + 4, 0xFF3A3A3A);
    }

    private static int lerpRgb(int from, int to, double t) {
        double clamped = Math.max(0.0, Math.min(1.0, t));
        int fr = (from >> 16) & 0xFF;
        int fg = (from >> 8) & 0xFF;
        int fb = from & 0xFF;
        int tr = (to >> 16) & 0xFF;
        int tg = (to >> 8) & 0xFF;
        int tb = to & 0xFF;
        int r = (int) Math.round(fr + (tr - fr) * clamped);
        int g = (int) Math.round(fg + (tg - fg) * clamped);
        int b = (int) Math.round(fb + (tb - fb) * clamped);
        return (r << 16) | (g << 8) | b;
    }

    private static String renderMitigationCell(String kind, float mitigation) {
        String label = com.bong.client.combat.ArmorProfileStore.kindLabel(kind);
        int pct = Math.round(mitigation * 100f);
        return label + "-" + pct + "%";
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return PANEL_WIDTH; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return currentHeight; }

    // ─── plan-inventory-hint-panel-v1 P2：测试专用访问器 ──────────────────────
    // draw()/computeRequiredHeight() 对非空 item 会触达 MinecraftClient.getInstance()
    // （无 MC 启动的单测环境会 NPE），故这些访问器只安全覆盖"空槽 hover"路径
    // （item==null 时 computeRequiredHeight 不touch MC，见既有惯例 PackContainerWindow.*ForTest()）。
    InventoryItem hoveredItemForTest() { return hoveredItem; }

    EquipSlotType hoveredSlotTypeForTest() { return hoveredSlotType; }

    int hoveredSlotWornCountForTest() { return hoveredSlotWornCount; }

    int currentHeightForTest() { return currentHeight; }
}
