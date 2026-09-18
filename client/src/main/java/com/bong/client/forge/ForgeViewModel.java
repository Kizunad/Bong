package com.bong.client.forge;

import com.bong.client.forge.state.BlueprintScrollStore;
import com.bong.client.forge.state.ForgeOutcomeStore;
import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.forge.state.ForgeStationStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import java.util.List;

/** 每 tick 读取一次的不可变锻造投影。 */
public record ForgeViewModel(ForgeStationStore.Snapshot station, ForgeSessionStore.Snapshot session,
                            List<BlueprintScrollStore.Entry> blueprints, int page,
                            InventoryModel inventory, ForgeOutcomeStore.Snapshot outcome, long inventoryRevision) {
    public ForgeViewModel { blueprints = List.copyOf(blueprints); }

    public static ForgeViewModel snapshot() {
        return new ForgeViewModel(ForgeStationStore.snapshot(), ForgeSessionStore.snapshot(),
            BlueprintScrollStore.entries(), BlueprintScrollStore.currentIndex(),
            InventoryStateStore.snapshot(), ForgeOutcomeStore.lastOutcome(), InventoryStateStore.revision());
    }

    public BlueprintScrollStore.Entry blueprint() {
        if (session.active()) return blueprints.stream().filter(entry -> entry.id().equals(session.blueprintId())).findFirst().orElse(null);
        if (!preparedMaterials().isEmpty()) return blueprints.stream()
            .filter(entry -> entry.id().equals(inventory.preparationRecipeId())).findFirst().orElse(null);
        return page >= 0 && page < blueprints.size() ? blueprints.get(page) : null;
    }

    public List<InventoryItem> preparedMaterials() {
        return station.pos() != null && station.pos().equals(inventory.preparationStation())
            ? inventory.preparedMaterials() : List.of();
    }

    public List<InventoryItem> materials() {
        return java.util.stream.Stream.concat(inventory.gridItems().stream().map(InventoryModel.GridEntry::item), inventory.hotbar().stream())
            .filter(item -> item != null && !item.isEmpty()).distinct().toList();
    }

    public String stepLabel() {
        if (!session.active()) return "准备投料";
        return stepLabel(session.currentStep());
    }

    public static String stepLabel(String step) {
        return switch (step) {
            case "billet" -> "制坯";
            case "tempering" -> "淬炼";
            case "inscription" -> "铭文";
            case "consecration" -> "开光";
            default -> "锻造完成";
        };
    }
}
