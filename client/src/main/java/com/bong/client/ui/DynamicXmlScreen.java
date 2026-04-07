package com.bong.client.ui;

import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.parsing.UIModel;
import net.minecraft.text.Text;

import java.util.Objects;

final class DynamicXmlScreen extends BaseOwoScreen<FlowLayout> {
    private final UIModel model;

    DynamicXmlScreen(String screenId, UIModel model) {
        super(Text.literal(screenId == null || screenId.isBlank() ? "动态界面" : screenId));
        this.model = Objects.requireNonNull(model, "model");
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return model.createAdapter(FlowLayout.class, this);
    }

    @Override
    protected void build(FlowLayout rootComponent) {
    }
}
