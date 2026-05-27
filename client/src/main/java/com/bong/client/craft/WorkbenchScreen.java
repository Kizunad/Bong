package com.bong.client.craft;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.client.MinecraftClient;
import net.minecraft.sound.SoundEvents;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.List;
import java.util.function.Consumer;
import java.util.stream.Collectors;

/**
 * plan-workbench-recipes-v1 P3.1 -- 末法制作台 UI。
 *
 * <p>复用 CraftScreen 框架（三栏布局 + CraftStore 状态）,
 * 但 <b>只展示 station == "workbench" 的配方</b>（严格隔离）。
 * 标题为「末法制作台」。</p>
 *
 * <p>由 WorkbenchScreenBootstrap 在收到 WorkbenchOpenPayload 时打开。</p>
 */
public final class WorkbenchScreen extends BaseOwoScreen<FlowLayout> {
    static final Text TITLE = Text.literal("末法制作台");

    private CraftRecipeListWidget recipeList;
    private CraftMaterialGrid materialGrid;
    private CraftOutputPreview outputPreview;
    private CraftActionBar actionBar;
    private LabelComponent subtitle;

    private String selectedId;
    private int flashTicks;
    private long lastTickSoundElapsed = -1;

    private final Consumer<List<CraftRecipe>> recipeListener = recipes -> scheduleRefresh();
    private final Consumer<CraftSessionStateView> sessionListener = state -> scheduleRefresh();
    private final Consumer<CraftStore.CraftOutcomeEvent> outcomeListener = event -> {
        if (event.kind() == CraftStore.CraftOutcomeEvent.Kind.COMPLETED) {
            flashTicks = 6;
            playCompleteSound();
        }
        scheduleRefresh();
    };
    private final Consumer<CraftStore.RecipeUnlockedEvent> unlockListener = event -> scheduleRefresh();
    private final Consumer<InventoryModel> inventoryListener = inventory -> scheduleRefresh();

    public WorkbenchScreen() {
        super(TITLE);
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

        FlowLayout panel = Containers.verticalFlow(
            Sizing.fixed(CraftScreenLayout.PANEL_W),
            Sizing.fixed(CraftScreenLayout.PANEL_H));
        panel.surface(Surface.flat(0xFF0D0D15).and(Surface.outline(0xFF4A4050)));
        panel.padding(Insets.of(6));
        panel.gap(4);
        panel.child(buildHeader());

        FlowLayout columns = Containers.horizontalFlow(
            Sizing.fill(100), Sizing.fixed(CraftScreenLayout.BODY_H));
        columns.gap(4);
        recipeList = new CraftRecipeListWidget(id -> {
            selectedId = id;
            refreshRightPanel();
        });
        materialGrid = new CraftMaterialGrid();
        outputPreview = new CraftOutputPreview();
        columns.child(recipeList.root());
        columns.child(materialGrid.root());
        columns.child(outputPreview.root());
        panel.child(columns);

        actionBar = new CraftActionBar(
            () -> actionBar.setQuantityToMax(), this::startCraft, this::refreshAll);
        panel.child(actionBar.root());

        root.child(panel);
        attachListeners();
        refreshAll();
    }

    @Override
    public void removed() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null && CraftStore.sessionState().active()) {
            ClientRequestSender.sendCraftCancel();
        }
        CraftStore.removeRecipeListener(recipeListener);
        CraftStore.removeSessionListener(sessionListener);
        CraftStore.removeOutcomeListener(outcomeListener);
        CraftStore.removeUnlockListener(unlockListener);
        InventoryStateStore.removeListener(inventoryListener);
        super.removed();
    }

    @Override
    public void tick() {
        super.tick();
        CraftSessionStateView state = CraftStore.sessionState();
        if (state.active()) {
            long elapsed = state.elapsedTicks();
            if (elapsed > 0 && elapsed % 20 == 0 && elapsed != lastTickSoundElapsed) {
                lastTickSoundElapsed = elapsed;
                playTickSound();
            }
        } else {
            lastTickSoundElapsed = -1;
        }
        if (flashTicks > 0) {
            flashTicks--;
            refreshOutputOnly();
        }
    }

    @Override
    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        if (keyCode == GLFW.GLFW_KEY_ESCAPE) {
            close();
            return true;
        }
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    /** 返回当前 Store 里 station=="workbench" 的配方子集。 */
    static List<CraftRecipe> workbenchRecipes() {
        return CraftStore.recipes().stream()
            .filter(CraftRecipe::isWorkbenchRecipe)
            .collect(Collectors.toList());
    }

    private FlowLayout buildHeader() {
        FlowLayout header = Containers.horizontalFlow(
            Sizing.fill(100), Sizing.fixed(CraftScreenLayout.HEADER_H));
        header.verticalAlignment(VerticalAlignment.CENTER);
        header.child(label("§f§l末法制作台", 0xFFFFFFFF));
        header.child(Containers.horizontalFlow(Sizing.fill(100), Sizing.content()));
        subtitle = label("Esc 关闭 · 双击快速制作", 0xFFA8A8B8);
        header.child(subtitle);
        return header;
    }

    private void attachListeners() {
        CraftStore.addRecipeListener(recipeListener);
        CraftStore.addSessionListener(sessionListener);
        CraftStore.addOutcomeListener(outcomeListener);
        CraftStore.addUnlockListener(unlockListener);
        InventoryStateStore.addListener(inventoryListener);
    }

    private void refreshAll() {
        if (recipeList == null || materialGrid == null
            || outputPreview == null || actionBar == null) {
            return;
        }
        InventoryModel inventory = InventoryStateStore.snapshot();
        ensureSelection();
        CraftRecipe selected = currentRecipe();
        recipeList.setSelectedId(selectedId);
        recipeList.refresh(inventory);
        CraftSessionStateView session = CraftStore.sessionState();
        actionBar.refresh(selected, inventory, session);
        materialGrid.refresh(selected, inventory, session, actionBar.quantity());
        outputPreview.refresh(selected, flashTicks);
        if (subtitle != null) {
            List<CraftRecipe> wb = workbenchRecipes();
            int known = wb.size();
            int craftable = selected == null ? 0
                : CraftInventoryCounter.maxCraftable(selected, inventory);
            subtitle.text(Text.literal(
                "Esc 关闭 · 配方 " + known + " · 当前可做 x" + craftable));
        }
    }

    private void refreshRightPanel() {
        if (materialGrid == null || outputPreview == null || actionBar == null) {
            return;
        }
        InventoryModel inventory = InventoryStateStore.snapshot();
        CraftRecipe selected = currentRecipe();
        CraftSessionStateView session = CraftStore.sessionState();
        actionBar.refresh(selected, inventory, session);
        materialGrid.refresh(selected, inventory, session, actionBar.quantity());
        outputPreview.refresh(selected, flashTicks);
        if (subtitle != null) {
            List<CraftRecipe> wb = workbenchRecipes();
            int known = wb.size();
            int craftable = selected == null ? 0
                : CraftInventoryCounter.maxCraftable(selected, inventory);
            subtitle.text(Text.literal(
                "Esc 关闭 · 配方 " + known + " · 当前可做 x" + craftable));
        }
    }

    private void refreshOutputOnly() {
        if (outputPreview != null) {
            outputPreview.refresh(currentRecipe(), flashTicks);
        }
    }

    private void ensureSelection() {
        List<CraftRecipe> wb = workbenchRecipes();
        if (selectedId != null && wb.stream().anyMatch(r -> r.id().equals(selectedId))) {
            return;
        }
        selectedId = wb.stream()
            .filter(CraftRecipe::unlocked)
            .findFirst()
            .or(() -> wb.stream().findFirst())
            .map(CraftRecipe::id)
            .orElse(null);
    }

    private CraftRecipe currentRecipe() {
        if (selectedId == null) return null;
        return workbenchRecipes().stream()
            .filter(r -> r.id().equals(selectedId))
            .findFirst()
            .orElse(null);
    }

    private void startCraft(int quantity) {
        CraftRecipe selected = currentRecipe();
        if (selected == null) return;
        ClientRequestSender.sendCraftStart(selected.id(), Math.max(1, quantity));
        playTickSound();
    }

    private void scheduleRefresh() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null) {
            client.execute(this::refreshAll);
        } else {
            refreshAll();
        }
    }

    private static void playTickSound() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null) {
            client.player.playSound(SoundEvents.BLOCK_ANVIL_USE, 0.1F, 1.5F);
        }
    }

    private static void playCompleteSound() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.player != null) {
            client.player.playSound(SoundEvents.ENTITY_PLAYER_LEVELUP, 0.2F, 1.5F);
        }
    }

    private static LabelComponent label(String text, int color) {
        LabelComponent label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        return label;
    }
}
