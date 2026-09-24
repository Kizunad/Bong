package com.bong.client.craft;

import com.bong.client.ui.intent.UiIntentResult;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.container.ScrollContainer;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import net.minecraft.client.MinecraftClient;
import net.minecraft.sound.SoundEvents;
import net.minecraft.text.Text;

/** 随身与工位制作共用窗口内容，隐藏不会解绑状态。 */
public final class CraftWindowContent {
    private final CraftWindows windows;
    private final FlowLayout root;
    private final FlowLayout body;
    private final FlowLayout details;
    private final ScrollContainer<FlowLayout> detailScroll;
    private final LabelComponent subtitle;
    private final CraftRecipeListWidget recipes;
    private final CraftMaterialGrid materials;
    private final CraftOutputPreview output = new CraftOutputPreview();
    private final CraftActionBar actions;
    private String selectedId;
    private CraftContext context;
    private int flashTicks;
    private long lastOutcomeRevision = -1;
    private long lastTickSoundElapsed = -1;
    private int layoutWidth = -1;
    private int layoutHeight = -1;
    private boolean available;
    private boolean busy;
    private CraftRecipe renderedRecipe;
    private com.bong.client.inventory.model.InventoryModel renderedInventory;
    private int renderedQuantity;
    private int renderedFlash;

    public CraftWindowContent(FlowLayout host, CraftWindows windows) {
        this.windows = windows;
        root = host;
        root.padding(Insets.of(4));
        root.gap(4);
        subtitle = Components.label(Text.empty());
        subtitle.color(Color.ofArgb(0xFF9CAFA9));
        subtitle.sizing(Sizing.fill(100), Sizing.fixed(12));
        root.child(subtitle);
        body = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(260));
        body.gap(6);
        recipes = new CraftRecipeListWidget(id -> {
            if (windows.busy() || !windows.model().inventory().craftMaterials().isEmpty()) {
                refresh(windows.model());
                return;
            }
            selectedId = id;
            refresh(windows.model());
        }, recipe -> windows.context().accepts(recipe)
            || recipe.id().equals(windows.model().inventory().craftRecipeId()));
        body.child(recipes.root());
        details = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        details.gap(6);
        materials = new CraftMaterialGrid(instanceId -> {
            if (selectedId != null) windows.material(selectedId, instanceId, true,
                windows.model().inventoryRevision());
        });
        details.child(materials.root());
        details.child(output.root());
        detailScroll = Containers.verticalScroll(Sizing.fixed(300), Sizing.fill(100), details);
        detailScroll.id("craft-detail-scroll");
        detailScroll.scrollbarThiccness(3);
        body.child(detailScroll);
        root.child(body);
        actions = new CraftActionBar(windows::returnAll, this::start, () -> refresh(windows.model()));
        root.child(actions.root());
        windows.listen(this::refresh);
        // 重开时仅投影当前状态，不能重播此前的制作结果。
        lastOutcomeRevision = windows.model().revision();
        refresh(windows.model());
    }

    public void layout(int width, int height) {
        if (width == layoutWidth && height == layoutHeight) return;
        layoutWidth = width;
        layoutHeight = height;
        int innerWidth = Math.max(1, width - 8);
        int bodyHeight = Math.max(1, height - 50);
        int listWidth = Math.min(170, Math.max(98, innerWidth / 3));
        int detailWidth = Math.max(1, innerWidth - listWidth - 6);
        body.verticalSizing(Sizing.fixed(bodyHeight));
        recipes.layout(listWidth, bodyHeight);
        detailScroll.sizing(Sizing.fixed(detailWidth), Sizing.fixed(bodyHeight));
        materials.layout(detailWidth - 4);
        output.layout(detailWidth - 4);
    }

    public void tick() {
        if (available != windows.available() || busy != windows.busy()) refresh(windows.model());
        var session = windows.model().session();
        if (session.active()) {
            long elapsed = session.elapsedTicks();
            if (elapsed > 0 && elapsed % 20 == 0 && elapsed != lastTickSoundElapsed) {
                lastTickSoundElapsed = elapsed;
                playTickSound();
            }
        } else lastTickSoundElapsed = -1;
        if (flashTicks > 0 && --flashTicks == 0) refresh(windows.model());
    }

    private CraftRecipe selected() {
        return selectedId == null ? null : windows.model().recipe(selectedId).orElse(null);
    }

    private void refresh(CraftScreenViewModel model) {
        if (!windows.context().equals(context)) {
            context = windows.context();
            selectedId = null;
        }
        if (!model.inventory().craftMaterials().isEmpty()) selectedId = model.inventory().craftRecipeId();
        if (model.session().active()) {
            model.session().recipeId().flatMap(model::recipe).filter(context::accepts)
                .ifPresent(recipe -> selectedId = recipe.id());
        }
        if (selected() == null || (model.inventory().craftMaterials().isEmpty() && !context.accepts(selected()))) {
            selectedId = model.recipes().stream().filter(context::accepts).filter(CraftRecipe::unlocked)
                .findFirst().or(() -> model.recipes().stream().filter(context::accepts).findFirst())
                .map(CraftRecipe::id).orElse(null);
        }
        if (model.change() == CraftScreenViewModel.Change.OUTCOME && model.revision() > lastOutcomeRevision) {
            lastOutcomeRevision = model.revision();
            CraftOutcomeFeedback.apply(model, ticks -> flashTicks = ticks,
                CraftOutcomeFeedback::playDefaultCompleteSound, () -> {});
        }
        available = windows.available();
        busy = windows.busy();
        var selected = selected();
        recipes.setSelectedId(selectedId);
        recipes.refresh(model.recipes(), model.inventory(), model.skills());
        actions.refresh(selected, model.inventory(), model.session(), model.skills());
        actions.availability(available && selected != null && context.accepts(selected), busy);
        if (selected != renderedRecipe || !model.inventory().equals(renderedInventory)
            || actions.quantity() != renderedQuantity) {
            materials.refresh(selected, model.inventory(), model.session(), actions.quantity());
            renderedInventory = model.inventory();
            renderedQuantity = actions.quantity();
        } else materials.refreshProgress(selected, model.session());
        if (selected != renderedRecipe || flashTicks != renderedFlash) {
            output.refresh(selected, flashTicks);
            renderedFlash = flashTicks;
        }
        renderedRecipe = selected;
        String location = context.workbench() == null ? "随身" : "工位 " + context.workbench().x()
            + ", " + context.workbench().y() + ", " + context.workbench().z();
        subtitle.text(Text.literal(!available ? "工位已不可用" : location));
    }

    public boolean acceptsDrop(double x, double y, String templateId) {
        return selectedId != null && !windows.busy() && windows.available()
            && detailScroll.isInBoundingBox(x, y) && templateId.equals(materials.materialAt(x, y));
    }

    public UiIntentResult drop(long instanceId) {
        return windows.material(selectedId, instanceId, false,
            windows.model().inventoryRevision());
    }

    private void start(int quantity) {
        if (selectedId != null && windows.start(selectedId, quantity).kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            playTickSound();
        }
    }

    private static void playTickSound() {
        var client = MinecraftClient.getInstance();
        if (client != null && client.player != null) client.player.playSound(SoundEvents.BLOCK_ANVIL_USE, 0.1F, 1.5F);
    }
}
