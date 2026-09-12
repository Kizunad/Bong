package com.bong.client.mixin;

import io.wispforest.owo.ui.component.LabelComponent;
import net.minecraft.text.OrderedText;
import net.minecraft.text.Style;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.List;

/** owo 0.11.2 对空文本仍读取第 -1 行；悬停和点击均经过此入口。 */
@Mixin(value = LabelComponent.class, remap = false)
public abstract class MixinLabelComponent {
    @Shadow protected List<OrderedText> wrappedText;

    @Inject(method = "styleAt", at = @At("HEAD"), cancellable = true)
    private void bong$emptyLabelHasNoTextStyle(int mouseX, int mouseY, CallbackInfoReturnable<Style> cir) {
        if (wrappedText.isEmpty()) {
            cir.setReturnValue(null);
        }
    }
}
