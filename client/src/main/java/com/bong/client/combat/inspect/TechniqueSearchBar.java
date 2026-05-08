package com.bong.client.combat.inspect;

import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.TextBoxComponent;
import io.wispforest.owo.ui.core.Sizing;

import java.util.function.Consumer;

/** Search input for the techniques tab. */
public final class TechniqueSearchBar {
    private final TextBoxComponent input;
    private String query = "";

    public TechniqueSearchBar(Consumer<String> onChanged) {
        input = Components.textBox(Sizing.fixed(176));
        input.text("");
        input.onChanged().subscribe(value -> {
            query = value == null ? "" : value;
            if (onChanged != null) onChanged.accept(query);
        });
    }

    public TextBoxComponent component() {
        return input;
    }

    public String query() {
        return query;
    }
}
