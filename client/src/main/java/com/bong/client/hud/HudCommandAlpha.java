package com.bong.client.hud;

final class HudCommandAlpha {
    private HudCommandAlpha() {
    }

    static HudRenderCommand withAlpha(HudRenderCommand command, double alphaFactor) {
        if (command == null) {
            return null;
        }
        double factor = Math.max(0.0, Math.min(1.0, Double.isFinite(alphaFactor) ? alphaFactor : 1.0));
        int color = multiplyAlpha(command.color(), factor);
        if (command.isText()) {
            return HudRenderCommand.text(command.layer(), command.text(), command.x(), command.y(), color);
        }
        if (command.isScaledText()) {
            return HudRenderCommand.scaledText(
                command.layer(), command.text(), command.x(), command.y(), color, command.textScale()
            );
        }
        if (command.isRect()) {
            return HudRenderCommand.rect(command.layer(), command.x(), command.y(), command.width(), command.height(), color);
        }
        if (command.isScreenTint()) {
            return HudRenderCommand.screenTint(command.layer(), color);
        }
        if (command.isEdgeVignette()) {
            return HudRenderCommand.edgeVignette(command.layer(), color);
        }
        if (command.isEdgeInkWash()) {
            return HudRenderCommand.edgeInkWash(command.layer(), color);
        }
        if (command.isToast()) {
            return HudRenderCommand.toast(command.layer(), command.text(), command.x(), command.y(), color);
        }
        if (command.isEdgeIndicator()) {
            return HudRenderCommand.edgeIndicator(
                command.layer(), command.text(), command.x(), command.y(), color, command.intensity()
            );
        }
        if (command.isTexturedRect()) {
            return HudRenderCommand.texture(
                command.layer(), command.texturePath(), command.x(), command.y(), command.width(), command.height(), color
            );
        }
        if (command.isItemTexture()) {
            return command;
        }
        return command;
    }

    private static int multiplyAlpha(int color, double factor) {
        int originalAlpha = (color >>> 24) == 0 ? 0xFF : (color >>> 24);
        int alpha = (int) Math.round(originalAlpha * factor);
        return (Math.max(0, Math.min(255, alpha)) << 24) | (color & 0x00FFFFFF);
    }
}
