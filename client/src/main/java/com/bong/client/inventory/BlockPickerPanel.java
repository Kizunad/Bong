package com.bong.client.inventory;

import com.bong.client.block.BlockPickerCatalog;
import com.bong.client.block.BlockPickerFilter;
import com.bong.client.block.BlockVanillaIconMap;
import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.component.TextBoxComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.CursorStyle;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.item.ItemStack;
import net.minecraft.text.Text;

import java.util.List;
import java.util.Optional;

/**
 * plan-worldgen-v4 P5 §8.1#5 — dev-only 方块审阅面板（owo 浮窗组件，非独立 Screen）。
 *
 * <p>仿 {@link LootContainerPanel} 模式挂进 {@link InspectScreen} 的 outerRow；仅当
 * {@code BlockPickerGate.isDevOrCreative()} 时挂载（对齐 HUD conditional display：非 dev
 * 直接不挂载，而非灰态）。每个条目展示一个 vanilla 方块图标 + 短名，点击发
 * {@code block_picker_give} C2S，让 server 把对应 vanilla 方块放进背包（**非生产
 * gameplay**）。</p>
 *
 * <p>给予逻辑在静态 {@link #giveBlock(String, int)} 暴露，便于 headless e2e 单测直接
 * 锁定「点击 → 真发 BlockPickerActionV1 payload」而无需 owo 渲染上下文。</p>
 */
public final class BlockPickerPanel {
    private static final int BG_COLOR = 0xFF0D1510;
    private static final int BORDER_COLOR = 0xFF40504A;
    private static final int ROW_HEIGHT = 18;
    private static final int PANEL_WIDTH = 150;
    private static final int LIST_HEIGHT = ROW_HEIGHT * 10;
    /** 单次给予数量（一格上限的一半，方便审阅时小批量摆放）。 */
    static final int DEFAULT_GIVE_COUNT = 1;

    private final List<String> blockIds;

    private FlowLayout container;
    private FlowLayout rows;
    private TextBoxComponent searchBox;
    private LabelComponent statusLabel;

    /**
     * @param blockIds 已排序的 vanilla 方块短名清单（生产传
     *                 {@link BlockPickerCatalog#vanillaBlockShortNames()}）
     */
    public BlockPickerPanel(List<String> blockIds) {
        this.blockIds = blockIds == null ? List.of() : List.copyOf(blockIds);
    }

    /** 便捷构造：从注册表枚举全 vanilla 方块（游戏内）。 */
    public static BlockPickerPanel fromVanillaRegistry() {
        return new BlockPickerPanel(BlockPickerCatalog.vanillaBlockShortNames());
    }

    /** 构建浮窗 UI，返回 root FlowLayout 供挂进 outerRow。 */
    public FlowLayout build() {
        container = Containers.verticalFlow(Sizing.content(), Sizing.content());
        container.surface(Surface.flat(BG_COLOR).and(Surface.outline(BORDER_COLOR)));
        container.padding(Insets.of(6));
        container.gap(3);
        container.horizontalAlignment(HorizontalAlignment.CENTER);

        container.child(Components.label(Text.literal("§a方块审阅 §8(dev)"))
            .horizontalTextAlignment(HorizontalAlignment.CENTER));

        searchBox = Components.textBox(Sizing.fixed(PANEL_WIDTH - 12));
        searchBox.text("");
        searchBox.onChanged().subscribe(value -> refreshRows(value));
        container.child(searchBox);

        rows = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        rows.gap(1);
        var scroll = Containers.verticalScroll(Sizing.fixed(PANEL_WIDTH), Sizing.fixed(LIST_HEIGHT), rows);
        scroll.scrollbarThiccness(3);
        container.child(scroll);

        statusLabel = Components.label(Text.literal(""));
        statusLabel.color(Color.ofArgb(0xFF889088));
        container.child(statusLabel);

        refreshRows("");
        return container;
    }

    /** root FlowLayout（build() 之后可用）。 */
    public FlowLayout container() {
        return container;
    }

    private void refreshRows(String query) {
        if (rows == null) {
            return;
        }
        rows.clearChildren();
        List<String> matches = BlockPickerFilter.filter(blockIds, query);
        if (matches.isEmpty()) {
            rows.child(Components.label(Text.literal("§8无匹配方块")));
        } else {
            for (String blockId : matches) {
                rows.child(buildRow(blockId));
            }
        }
        if (statusLabel != null) {
            statusLabel.text(Text.literal(String.format("§8%d / %d 块", matches.size(), blockIds.size())));
        }
    }

    private FlowLayout buildRow(String blockId) {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(ROW_HEIGHT));
        row.verticalAlignment(VerticalAlignment.CENTER);
        row.gap(3);
        row.padding(Insets.of(1, 1, 2, 2));
        row.cursorStyle(CursorStyle.HAND);

        Optional<ItemStack> icon = BlockVanillaIconMap.createVanillaBlockStack(blockId);
        icon.ifPresent(stack -> row.child(Components.item(stack)));

        LabelComponent name = Components.label(Text.literal(blockId));
        name.color(Color.ofArgb(0xFFD8E4D0));
        name.maxWidth(PANEL_WIDTH - 30);
        row.child(name);

        row.tooltip(Text.literal("§7给予 §f" + DEFAULT_GIVE_COUNT + "× §avanilla:" + blockId));
        row.mouseDown().subscribe((mx, my, btn) -> {
            if (btn == 0) {
                giveBlock(blockId, DEFAULT_GIVE_COUNT);
                return true;
            }
            return false;
        });
        return row;
    }

    /**
     * plan-worldgen-v4 P5 §8.1#5 — 真发 give-block 请求（防僵尸 UI 的真接线点）。
     *
     * <p>校验 blockId 非空、count ∈ [1, {@link ClientRequestProtocol#BLOCK_PICKER_MAX_COUNT}]
     * 后通过 {@link ClientRequestSender#sendBlockPickerGive(String, int)} 发
     * {@code block_picker_give} C2S。非法入参返回 {@code false} 且不发包。</p>
     *
     * @return {@code true} 当且仅当真发出了一条 client_request
     */
    public static boolean giveBlock(String blockId, int count) {
        if (blockId == null || blockId.isBlank()) {
            return false;
        }
        if (count < 1 || count > ClientRequestProtocol.BLOCK_PICKER_MAX_COUNT) {
            return false;
        }
        ClientRequestSender.sendBlockPickerGive(blockId, count);
        return true;
    }
}
