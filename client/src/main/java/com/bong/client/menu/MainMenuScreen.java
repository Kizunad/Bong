package com.bong.client.menu;

import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.option.OptionsScreen;
import net.minecraft.text.Text;

/** XML 负责交互布局；背景、装饰字标和按钮皮肤由独立渲染器绘制。 */
public final class MainMenuScreen extends OwoXmlScreenHost<FlowLayout> {
    private enum Intent { ENTER, SETTINGS, EXIT }

    public MainMenuScreen() {
        super(Text.literal("末法残土"), FlowLayout.class, "main-menu");
    }

    @Override
    protected void bindTemplate(FlowLayout root) {
        MainMenuConfig config = MainMenuFlow.open();
        bind("menu-enter", Intent.ENTER);
        bind("menu-settings", Intent.SETTINGS);
        bind("menu-exit", Intent.EXIT);
        if (config == null) {
            status(Text.translatable("bong.menu.config_invalid"));
        } else if (config.serverAddress().isEmpty()) {
            status(Text.translatable("bong.menu.config_missing"));
        }
    }

    private void bind(String id, Intent intent) {
        ButtonComponent button = component(ButtonComponent.class, id);
        button.textShadow(false).renderer((context, value, delta) ->
            MainMenuBackdrop.drawCommandDecoration(context, value, value.isHovered() || value.isFocused()));
        button.onPress(ignored -> dispatch(intent));
    }

    private void dispatch(Intent intent) {
        switch (intent) {
            case ENTER -> {
                Text failure = MainMenuFlow.enter(client);
                if (client.currentScreen == this) {
                    status(failure);
                }
            }
            // 返回时创建新的 Screen scope，避免复用已 removed 的 owo 宿主。
            case SETTINGS -> client.setScreen(new OptionsScreen(new MainMenuScreen(), client.options));
            case EXIT -> client.scheduleStop();
        }
    }

    private void status(Text text) {
        label("menu-status").text(text);
    }

    @Override
    public void render(DrawContext context, int mouseX, int mouseY, float delta) {
        MainMenuFlow.BACKDROP.render(context, width, height, mouseX, mouseY);
        if (!hostReadyForTests()) {
            super.render(context, mouseX, mouseY, delta);
            return;
        }
        var brand = componentBoundsForPreview("menu-brand");
        MainMenuBackdrop.drawBrand(context, brand.x(), brand.y() + 4);
        super.render(context, mouseX, mouseY, delta);
        context.drawText(textRenderer, "BONG  /  0.1.0", 12, height - 14, 0xFF8A968D, false);
    }

    @Override
    public boolean shouldCloseOnEsc() {
        return false;
    }

    @Override
    public boolean shouldPause() {
        return false;
    }
}
