package com.bong.client.inventory.component;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.SlotContents;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.List;

/**
 * plan-layered-equip-v1 P4（决议 #12/#17）：装备槽组件，支持 worn 栈多层叠放 + held 单件渲染。
 *
 * <p>视听规格（§P4「装备面板布局 + worn 栈叠放渲染」）：
 * <ul>
 *   <li><b>worn 多层叠放</b>（栈序：尾=顶）：栈底 worn[0] 最下、栈顶 worn.last() 最上，每层向右下偏移 +2px/+2px；
 *       最多渲染 cap 层；下层被上层遮住大部、底边露 ~2px 示意被压住。</li>
 *   <li><b>栈顶高亮 + 可拖</b>：栈顶正常亮度 + 描边高亮 #FFE8B0（淡金边 1px），仅栈顶可拖下/卸下。</li>
 *   <li><b>下层 dim / 不可交互</b>：被压住下层叠 tint #000000 opacity 0.40（dim），不可拖。</li>
 *   <li><b>层数角标</b>：右上角 (x+14, y-2)，颜色 #FFD27F，7px（scale 0.7），文案 ×N（N≥2 才显示）。</li>
 *   <li><b>held 单独位</b>：held 物品渲染在右下角小图标位 (x+9, y+9)，叠在 worn 之上，8px 缩略图。</li>
 *   <li><b>双手另一手 disable</b>：被锁对侧手槽叠 tint #000000 opacity 0.55 + 灰色锁图标（中心 (x+5,y+5)）。</li>
 * </ul>
 */
public class EquipSlotComponent extends BaseComponent {
    public static final int SLOT_SIZE = 28;
    private static final int BG_COLOR = 0xFF1E1E1E;
    private static final int BORDER_COLOR = 0xFF4A4A4A;
    private static final int EMPTY_LABEL_COLOR = 0x60999999;
    private static final int HOVER_BORDER_COLOR = 0xFF999999;

    // worn 栈视听常量（与 §P4 视听规格一致）。
    static final int LAYER_OFFSET = 2;                 // 每层 +2px/+2px 偏移
    static final int TOP_HIGHLIGHT_COLOR = 0xFFFFE8B0; // 栈顶描边淡金
    static final int LOWER_DIM_COLOR = 0x66000000;     // 下层 dim：opacity 0.40 (0x66≈102/255)
    static final int BADGE_COLOR = 0xFFFFD27F;         // 层数角标淡金
    static final float BADGE_SCALE = 0.7f;             // 7px small font (vanilla 10px × 0.7)
    static final int HELD_ICON_SIZE = 8;               // held 缩略图 8px
    static final int HELD_BORDER_COLOR = 0xFFC040FF;   // held 武器边框（紫，与 HUD 武器槽一致）
    static final int DISABLE_TINT_COLOR = 0x8C000000;  // 双手锁对侧手：opacity 0.55 (0x8C≈140/255)
    static final int LOCK_ICON_COLOR = 0xFFB0B0B0;     // 灰色锁图标

    private final EquipSlotType slotType;
    private SlotContents contents = SlotContents.empty();
    // 双手武器锁定本（对侧手）槽 → 不可交互 + disable tint。
    private boolean disabledByTwoHand = false;
    private GridSlotComponent.HighlightState highlightState = GridSlotComponent.HighlightState.NONE;

    public EquipSlotComponent(EquipSlotType slotType) {
        this.slotType = slotType;
        this.sizing(Sizing.fixed(SLOT_SIZE), Sizing.fixed(SLOT_SIZE));
    }

    public EquipSlotType slotType() { return slotType; }

    /**
     * plan-inventory-hint-panel-v1 P2：{@code worn_cap} 静态规则客户端镜像
     * （server {@code worn_cap()} @ {@code inventory/mod.rs:636-643}——head/feet=2、chest/legs=3、
     * 手槽（held-only，worn 恒空）=0）。
     *
     * <p><b>只覆盖静态常量</b>：{@code worn_cap_bonus}（server mod.rs:659-661，P5 占位，
     * 当前恒返回 0）一旦未来接入境界/功法派生加成，无法纯 client 推算——届时动态加成类拒绝
     * 仍须走 P1 失败 toast，本方法/hover 面板不承诺动态结果（plan §P2 边界）。
     */
    static int wornCap(EquipSlotType slotType) {
        return switch (slotType) {
            case HEAD, FEET -> 2;
            case CHEST, LEGS -> 3;
            case MAIN_HAND, OFF_HAND, EXTRA_HAND_0, EXTRA_HAND_1 -> 0;
        };
    }

    public SlotContents contents() { return contents; }

    public void setContents(SlotContents contents) {
        this.contents = contents == null ? SlotContents.empty() : contents;
    }

    public void clearContents() { this.contents = SlotContents.empty(); }

    /** worn 栈顶件（决议 #12：唯一可拖/可卸），无则 null。 */
    public InventoryItem wornTop() { return contents.wornTop(); }

    /** held 持械件，无则 null。 */
    public InventoryItem held() { return contents.held(); }

    /** 该槽「代表件」：手槽 held / 身体槽 worn 栈顶（供旧单件交互逻辑取数）。 */
    public InventoryItem representative() { return contents.representative(); }

    public boolean isEmpty() { return contents.isEmpty(); }

    public void setDisabledByTwoHand(boolean disabled) { this.disabledByTwoHand = disabled; }

    public boolean isDisabledByTwoHand() { return disabledByTwoHand; }

    public void setHighlightState(GridSlotComponent.HighlightState state) {
        this.highlightState = state;
    }

    @Override
    public void draw(OwoUIDrawContext context, int mouseX, int mouseY, float partialTicks, float delta) {
        context.fill(x, y, x + SLOT_SIZE, y + SLOT_SIZE, BG_COLOR);

        int borderColor = hovered ? HOVER_BORDER_COLOR : BORDER_COLOR;
        GridSlotComponent.drawSlotBorder(context, x, y, SLOT_SIZE, SLOT_SIZE, borderColor);

        switch (highlightState) {
            case VALID -> context.fill(x + 1, y + 1, x + SLOT_SIZE - 1, y + SLOT_SIZE - 1, 0x3300CC44);
            case INVALID -> context.fill(x + 1, y + 1, x + SLOT_SIZE - 1, y + SLOT_SIZE - 1, 0x33CC2222);
            case DIMMED -> context.fill(x + 1, y + 1, x + SLOT_SIZE - 1, y + SLOT_SIZE - 1, 0x66000000);
            default -> {}
        }

        List<InventoryItem> worn = contents.worn();
        InventoryItem heldItem = contents.held();

        if (worn.isEmpty() && heldItem == null) {
            // 空槽：单字部位标签。
            var textRenderer = MinecraftClient.getInstance().textRenderer;
            String label = slotType.displayName().substring(0, 1);
            int textWidth = textRenderer.getWidth(label);
            int tx = x + (SLOT_SIZE - textWidth) / 2;
            int ty = y + (SLOT_SIZE - textRenderer.fontHeight) / 2;
            context.drawTextWithShadow(textRenderer, Text.literal(label), tx, ty, EMPTY_LABEL_COLOR);
        } else {
            drawWornStack(context, worn);
            if (heldItem != null) {
                drawHeld(context, heldItem);
            }
        }

        if (disabledByTwoHand) {
            drawTwoHandDisable(context);
        }
    }

    /** worn 栈：栈底→栈顶逐层叠放，下层 dim 露底边，栈顶高亮 + 层数角标。 */
    private void drawWornStack(OwoUIDrawContext context, List<InventoryItem> worn) {
        int n = worn.size();
        int inner = SLOT_SIZE - 4;
        for (int i = 0; i < n; i++) {
            boolean top = (i == n - 1);
            int ox = x + 2 + i * LAYER_OFFSET;
            int oy = y + 2 + i * LAYER_OFFSET;
            // 栈高时收一点尺寸保持在槽内（防越界）。
            int size = Math.max(8, inner - i * LAYER_OFFSET);
            GridSlotComponent.drawItemTexture(context, worn.get(i), ox, oy, size, size);
            if (!top) {
                // 下层 dim（被压住示意）。
                context.fill(ox, oy, ox + size, oy + size, LOWER_DIM_COLOR);
            }
        }
        if (n > 0) {
            // 栈顶描边高亮（淡金）。
            int topIdx = n - 1;
            int ox = x + 2 + topIdx * LAYER_OFFSET;
            int oy = y + 2 + topIdx * LAYER_OFFSET;
            int size = Math.max(8, inner - topIdx * LAYER_OFFSET);
            drawRectOutline(context, ox - 1, oy - 1, size + 2, size + 2, TOP_HIGHLIGHT_COLOR);
        }
        if (n >= 2) {
            drawLayerBadge(context, n);
        }
    }

    /** 层数角标 ×N（右上角，淡金，7px）。 */
    private void drawLayerBadge(OwoUIDrawContext context, int count) {
        var textRenderer = MinecraftClient.getInstance().textRenderer;
        String label = "×" + count;
        context.getMatrices().push();
        context.getMatrices().translate(x + 14.0f, y - 2.0f, 0.0f);
        context.getMatrices().scale(BADGE_SCALE, BADGE_SCALE, 1.0f);
        context.drawText(textRenderer, Text.literal(label), 0, 0, BADGE_COLOR, false);
        context.getMatrices().pop();
    }

    /** held 单独位（右下角 8px 缩略图，叠在 worn 之上 + 紫边）。 */
    private void drawHeld(OwoUIDrawContext context, InventoryItem heldItem) {
        int hx = x + 9;
        int hy = y + 9;
        GridSlotComponent.drawItemTexture(context, heldItem, hx, hy, HELD_ICON_SIZE, HELD_ICON_SIZE);
        drawRectOutline(context, hx - 1, hy - 1, HELD_ICON_SIZE + 2, HELD_ICON_SIZE + 2, HELD_BORDER_COLOR);
    }

    /** 双手另一手 disable：暗化 tint + 灰色锁图标。 */
    private void drawTwoHandDisable(OwoUIDrawContext context) {
        context.fill(x + 1, y + 1, x + SLOT_SIZE - 1, y + SLOT_SIZE - 1, DISABLE_TINT_COLOR);
        int cx = x + SLOT_SIZE / 2;
        int cy = y + SLOT_SIZE / 2;
        context.fill(cx - 4, cy, cx + 4, cy + 6, LOCK_ICON_COLOR);    // 锁身
        context.fill(cx - 2, cy - 4, cx + 2, cy, LOCK_ICON_COLOR);    // 锁梁
    }

    private static void drawRectOutline(OwoUIDrawContext context, int rx, int ry, int w, int h, int color) {
        context.fill(rx, ry, rx + w, ry + 1, color);
        context.fill(rx, ry + h - 1, rx + w, ry + h, color);
        context.fill(rx, ry, rx + 1, ry + h, color);
        context.fill(rx + w - 1, ry, rx + w, ry + h, color);
    }

    @Override
    protected int determineHorizontalContentSize(Sizing sizing) { return SLOT_SIZE; }

    @Override
    protected int determineVerticalContentSize(Sizing sizing) { return SLOT_SIZE; }
}
