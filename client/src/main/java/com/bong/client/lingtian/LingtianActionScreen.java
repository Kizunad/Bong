package com.bong.client.lingtian;

import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.CursorStyle;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;
import net.minecraft.util.math.BlockPos;

/**
 * plan-lingtian-v1 §1.2-§1.7 — 灵田动作触发入口（UI 切片 1）。
 *
 * <p>按 L 键打开：snapshot 玩家当前 crosshair 指向的方块坐标作为目标 plot。
 * 6 类动作（开垦 / 种植 / 补灵 / 收获 / 翻新 / 偷灵）以按钮形式列出，点击后通过
 * {@link ClientRequestSender} 发对应 intent。server 端已有校验（plot 存在 / 空 /
 * 熟 / 冷却 / 库存），失败会 warn，进度 HUD 条由 {@code LingtianSessionHud} 展示。
 *
 * <p>未做：开垦/翻新需要主手锄 {@code instance_id}，当前传 0 占位（server 会拒绝，
 * 调试用 — 待 InventoryStateStore 读主手锄后补）。
 */
public final class LingtianActionScreen extends BaseOwoScreen<FlowLayout> {
    private static final Text TITLE = Text.literal("灵田动作");
    private static final int PANEL_W = 280;
    private static final int PANEL_H = 280;

    private static final String[] SEED_PLANTS = {"ci_she_hao", "ning_mai_cao", "ling_mu_miao"};
    private static final String[] SEED_LABELS = {"§a刺舌蒿", "§a凝脉草", "§b灵木苗"};
    private static final String[] REPLENISH_SOURCES = {"zone", "bone_coin", "beast_core", "ling_shui"};
    private static final String[] REPLENISH_LABELS = {"§7区域抽吸 (8s)", "§e骨币 +0.8", "§d兽核 +2.0", "§b灵水 +0.3"};

    private final BlockPos target;

    public LingtianActionScreen(BlockPos target) {
        super(TITLE);
        this.target = target;
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout root) {
        root.surface(Surface.VANILLA_TRANSLUCENT);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        root.verticalAlignment(VerticalAlignment.CENTER);

        FlowLayout panel = Containers.verticalFlow(Sizing.fixed(PANEL_W), Sizing.fixed(PANEL_H));
        panel.surface(Surface.flat(0xFF101815).and(Surface.outline(0xFF4A6A40)));
        panel.padding(Insets.of(8));
        panel.gap(4);
        panel.horizontalAlignment(HorizontalAlignment.CENTER);

        panel.child(header());
        panel.child(targetLine());
        panel.child(divider());

        if (target == null) {
            LabelComponent hint = Components.label(Text.literal("§c请先注视一个方块后按 L"));
            panel.child(hint);
        } else {
            panel.child(actionRow("§e开垦", "空地 → 空 plot", () -> send(() ->
                ClientRequestSender.sendLingtianStartTill(target.getX(), target.getY(), target.getZ(), 0L, "manual"))));
            panel.child(expandablePlanting());
            panel.child(expandableReplenish());
            panel.child(actionRow("§a收获", "熟作物 → 入背包", () -> send(() ->
                ClientRequestSender.sendLingtianStartHarvest(target.getX(), target.getY(), target.getZ(), "manual"))));
            panel.child(actionRow("§6翻新", "贫瘠 plot → 重置", () -> send(() ->
                ClientRequestSender.sendLingtianStartRenew(target.getX(), target.getY(), target.getZ(), 0L))));
            panel.child(actionRow("§c偷灵", "吸他人 plot_qi", () -> send(() ->
                ClientRequestSender.sendLingtianStartDrainQi(target.getX(), target.getY(), target.getZ()))));
        }

        panel.child(divider());
        LabelComponent foot = Components.label(Text.literal("§7ESC 关闭 · 动作进度看中下进度条"));
        panel.child(foot);

        root.child(panel);
    }

    private FlowLayout header() {
        FlowLayout h = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        h.horizontalAlignment(HorizontalAlignment.CENTER);
        h.child(Components.label(Text.literal("§f§l灵田动作")));
        return h;
    }

    private FlowLayout targetLine() {
        FlowLayout h = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        h.horizontalAlignment(HorizontalAlignment.CENTER);
        String txt = target == null
            ? "§8目标：(未瞄准)"
            : String.format("§7目标：[%d, %d, %d]", target.getX(), target.getY(), target.getZ());
        h.child(Components.label(Text.literal(txt)));
        return h;
    }

    private FlowLayout expandablePlanting() {
        FlowLayout col = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        col.gap(2);
        col.child(labelRow("§a种植", "选 1 种种子播种（背包需有种子）"));
        FlowLayout seedRow = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        seedRow.gap(3);
        seedRow.horizontalAlignment(HorizontalAlignment.CENTER);
        for (int i = 0; i < SEED_PLANTS.length; i++) {
            final String plant = SEED_PLANTS[i];
            seedRow.child(button(SEED_LABELS[i], () -> send(() ->
                ClientRequestSender.sendLingtianStartPlanting(target.getX(), target.getY(), target.getZ(), plant))));
        }
        col.child(seedRow);
        return col;
    }

    private FlowLayout expandableReplenish() {
        FlowLayout col = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        col.gap(2);
        col.child(labelRow("§b补灵", "选 1 来源（骨币/兽核/灵水从背包扣）"));
        FlowLayout srcRow = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        srcRow.gap(2);
        srcRow.horizontalAlignment(HorizontalAlignment.CENTER);
        for (int i = 0; i < REPLENISH_SOURCES.length; i++) {
            final String src = REPLENISH_SOURCES[i];
            srcRow.child(button(REPLENISH_LABELS[i], () -> send(() ->
                ClientRequestSender.sendLingtianStartReplenish(target.getX(), target.getY(), target.getZ(), src))));
        }
        col.child(srcRow);
        return col;
    }

    private FlowLayout actionRow(String titleColored, String tip, Runnable onClick) {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        row.verticalAlignment(VerticalAlignment.CENTER);
        row.gap(6);
        row.child(button(titleColored, onClick));
        row.child(Components.label(Text.literal("§8" + tip)));
        return row;
    }

    private FlowLayout labelRow(String titleColored, String tip) {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        row.verticalAlignment(VerticalAlignment.CENTER);
        row.gap(6);
        row.child(Components.label(Text.literal(titleColored)));
        row.child(Components.label(Text.literal("§8" + tip)));
        return row;
    }

    private FlowLayout divider() {
        FlowLayout d = Containers.horizontalFlow(Sizing.fill(95), Sizing.fixed(1));
        d.surface(Surface.flat(0xFF4A6A40));
        return d;
    }

    private FlowLayout button(String text, Runnable onClick) {
        LabelComponent lbl = Components.label(Text.literal(text));
        lbl.color(Color.ofArgb(0xFFDCDCDC));
        lbl.cursorStyle(CursorStyle.HAND);
        FlowLayout wrap = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        wrap.padding(Insets.of(2, 2, 6, 6));
        wrap.surface(Surface.flat(0xFF1F2A1F).and(Surface.outline(0xFF5A8050)));
        wrap.cursorStyle(CursorStyle.HAND);
        wrap.child(lbl);
        wrap.mouseDown().subscribe((mx, my, btn) -> {
            if (btn == 0) { onClick.run(); return true; }
            return false;
        });
        return wrap;
    }

    private void send(Runnable action) {
        try {
            action.run();
        } catch (RuntimeException e) {
            // Sender.dispatch 在无 backend 时会 throw；调试用 log + 忽略。
            com.bong.client.BongClient.LOGGER.warn("[lingtian] intent send failed: {}", e.getMessage());
        }
        MinecraftClient.getInstance().setScreen(null);
    }
}
