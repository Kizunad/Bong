package com.bong.client.ui.preview;

import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.component.BodyInspectComponent;
import com.bong.client.inventory.model.*;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.inventory.state.PhysicalBodyStore;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.inventory.state.PlayerRaceIdentityStore;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.ui.ScreenTransitionController;
import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.ui.window.UiWindowRuntime;
import io.wispforest.owo.ui.core.Component;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;

/** 仅由显式启用的原生预览 harness 装配；使用真实模型和鼠标输入，循环状态是客户端夹具。 */
final class UiBodyModelPreviewScene implements UiPreviewScene {
    private UiWindowManager.WindowState window;
    private BodyInspectComponent body;
    private MeridianBody savedMeridians;
    private PhysicalBody savedPhysical;
    private PlayerStateViewModel savedPlayer;
    private String name;
    private MeridianBody fixtureMeridians;
    private PhysicalBody fixturePhysical;
    private PlayerStateViewModel fixturePlayer;
    private java.nio.file.Path frames;
    private int frameTick;

    @Override public boolean clientReady(MinecraftClient client) {
        return client.world != null && client.player != null && client.currentScreen == null
            && ScreenTransitionController.activeTransition() == null && PlayerRaceIdentityStore.formIdentityKnown();
    }
    @Override public void installFixture(UiPreviewConfig config) {
        frames=java.nio.file.Path.of(config.outputDir()).resolve("entrance-frames");
        UiWindowRuntime.beginPreview();
        savedMeridians=MeridianStateStore.snapshot();
        savedPhysical=PhysicalBodyStore.snapshot();
        savedPlayer=PlayerStateStore.snapshot();
    }
    @Override public Screen createScreen() {
        return new InspectScreen(InventoryStateStore.snapshot()) {
            @Override public void render(DrawContext context, int mouseX, int mouseY, float delta) {
                // 网络任务也可能在 tick 与 render 之间应用快照；夹具在真实绘制前覆盖。
                applyFixture();
                super.render(context, mouseX, mouseY, delta);
            }
        };
    }
    @Override public String selectedTemplateId(Screen screen) { return "body-inspect"; }
    @Override public boolean isReady(Screen screen) { return ((InspectScreen)screen).windowHostReadyForPreview(); }
    @Override public boolean initializationFailed(Screen screen) { return ((InspectScreen)screen).windowHostFailedForPreview(); }

    @Override public void prepareScreenshot(Screen screen, UiPreviewShot shot) {
        name=shot.name(); frameTick=0; body=null;
        var manager=UiWindowRuntime.manager();
        UiPreviewCleanup.run(manager.snapshot().stream()
            .map(state -> (Runnable) () -> manager.close(state.key()))
            .toArray(Runnable[]::new));
        var builder=MeridianBody.builder().realm("Induce").lifespanPreview(24,120,96,5,1,false);
        for(var ch:MeridianChannel.values()) builder.channel(new ChannelState(ch,10,8,
            ch==MeridianChannel.HT ? ChannelState.DamageLevel.SEVERED : ChannelState.DamageLevel.INTACT,
            ch==MeridianChannel.KI ? .35 : 0, .25, ch==MeridianChannel.PC));
        fixtureMeridians=builder.build();
        fixturePhysical=MockPhysicalData.create();
        fixturePlayer=PlayerStateViewModel.create("Induce","preview",name.contains("empty")?0:70,100,0,.5,
            PlayerStateViewModel.PowerBreakdown.empty(),PlayerStateViewModel.SocialSnapshot.empty(),
            "preview","内观",.5);
        tick();
        var layer=name.startsWith("physical") ? BodyInspectComponent.Layer.PHYSICAL : BodyInspectComponent.Layer.MERIDIAN;
        ((InspectScreen)screen).openBodyWindow(layer);
        window=manager.snapshot().get(manager.snapshot().size()-1);
        manager.settleAt(window.key(),new UiWindowManager.Rect(8,8,
            Math.min(640,shot.expectedLogicalWidth()-16),Math.min(450,shot.expectedLogicalHeight()-44)));
        render(screen);
        body=UiWindowRuntime.body(layer);
        require(body.anatomical(),"当前账号未同步人形身份");
        if(name.startsWith("view-")) {
            body.overview();
            float yaw = switch(name) { case "view-back" -> 180; case "view-left" -> 90; case "view-right" -> -90; default -> 0; };
            float pitch = switch(name) { case "view-top" -> 85; case "view-bottom" -> -85; default -> 0; };
            body.camera().focus(new net.minecraft.util.math.Vec3d(.5,.5,.5),1,yaw,pitch,false);
            return;
        }
        if(name.contains("animation")) return;
        var list=component("body-targets");
        click(screen,list.x()+14,list.y()+12);
        render(screen);
        if(layer==BodyInspectComponent.Layer.PHYSICAL) require(body.selectedPart()==BodyPart.HEAD,"肢体列表未选择头部");
        else require(body.selectedChannel()==MeridianChannel.LU,"经脉列表未选择肺脉");
        if(name.contains("overview") || name.contains("minimum")) body.overview();
        if(name.contains("back")) body.setSelectedChannel(MeridianChannel.DU);
        if(name.contains("leg")) body.setSelectedPart(BodyPart.LEFT_CALF);
        if(name.contains("details")) {
            var controls=component("body-controls");
            click(screen,controls.x()+controls.width()*7/8.0,controls.y()+8);
        }
        if(name.contains("overlap")) {
            var selected=body.selectedChannel();
            ((InspectScreen)screen).openBodyWindow(BodyInspectComponent.Layer.PHYSICAL);
            var upper=manager.snapshot().get(manager.snapshot().size()-1);
            manager.settleAt(upper.key(),window.bounds());
            render(screen);
            require(UiWindowRuntime.bodyAt(body.x()+body.width()/2.0,body.y()+body.height()/2.0)!=body,
                "被覆盖的模型仍可接收点击");
            require(body.selectedChannel()==selected,"被覆盖的模型选择被修改");
        }
    }

    private void applyFixture() {
        if(fixtureMeridians==null) return;
        MeridianStateStore.replace(fixtureMeridians);
        PhysicalBodyStore.replace(fixturePhysical);
        PlayerStateStore.replace(fixturePlayer);
    }
    @Override public void tick() {
        applyFixture();
        if(body!=null && name.contains("animation") && frameTick++<40 && frameTick%2==0) {
            try {
                java.nio.file.Files.createDirectories(frames);
                try(var image=net.minecraft.client.util.ScreenshotRecorder.takeScreenshot(MinecraftClient.getInstance().getFramebuffer())) {
                    image.writeTo(frames.resolve(String.format(java.util.Locale.ROOT,"frame-%03d.png",frameTick)));
                }
            } catch(java.io.IOException error) { throw new java.io.UncheckedIOException(error); }
        }
    }

    @Override public void validateGeometry(Screen screen, UiPreviewShot shot) {
        tick();
        require(body.failure()==null,"内观模型渲染失败："+body.failure());
        require(body.hasModelGeometry(),"未捕获到实际玩家模型的部件姿态");
        require(!body.camera().moving(),"入场后模型没有停稳");
        if(name.contains("overlap")) return;
        for(String id:new String[]{"body-model","body-summary","body-targets","body-details","body-controls"}) {
            var c=component(id);
            require(c.width()>0 && c.height()>0 && c.x()>=window.bounds().x() && c.y()>=window.bounds().y()
                && c.x()+c.width()<=window.bounds().x()+window.bounds().width()
                && c.y()+c.height()<=window.bounds().y()+window.bounds().height(),"窗口控件越界："+id);
        }
        var selection=body.selectedChannel();
        double x=body.x()+body.width()/2.0,y=body.y()+body.height()/2.0;
        float yaw=body.camera().yaw();
        screen.mouseClicked(x,y,0); screen.mouseDragged(x+20,y,0,20,0); screen.mouseReleased(x+20,y,0);
        require(body.camera().yaw()!=yaw,"拖拽没有旋转模型");
        require(body.selectedChannel()==selection,"拖拽误触经脉选择");
        float zoom=body.camera().zoom();
        screen.mouseScrolled(x,y,1);
        require(body.camera().zoom()>zoom,"滚轮没有放大模型");
        UiWindowRuntime.manager().minimize(window.key());
        UiWindowRuntime.manager().restore(window.key());
        require(body.selectedChannel()==selection,"恢复丢失选择");
        UiWindowRuntime.manager().pin(window.key(),true);
        require(window.pinned(),"内观窗口没有固定到 HUD");
    }
    private Component component(String id) { return UiWindowRuntime.windowContentForPreview(window.key()).childById(Component.class,id); }
    private static void click(Screen screen,double x,double y) { screen.mouseClicked(x,y,0); screen.mouseReleased(x,y,0); }
    private static void render(Screen screen) {
        var client=MinecraftClient.getInstance();
        var context=new DrawContext(client,client.getBufferBuilders().getEntityVertexConsumers());
        screen.render(context,-1,-1,0); context.draw();
    }
    private static void require(boolean condition,String reason) { if(!condition) throw new IllegalStateException(reason); }
    @Override public void cleanup() {
        fixtureMeridians=null;
        UiWindowRuntime.manager().reset();
        UiWindowRuntime.endPreview();
        MeridianStateStore.replace(savedMeridians);
        PhysicalBodyStore.replace(savedPhysical);
        PlayerStateStore.replace(savedPlayer);
    }
}
