package com.bong.client.cultivation;

import com.bong.client.inventory.component.BodyInspectComponent;
import com.bong.client.combat.inspect.StatusPanelExtension;
import com.bong.client.inventory.model.*;
import com.bong.client.inventory.state.MeridianStateStore;
import com.bong.client.inventory.state.PhysicalBodyStore;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.ui.intent.UiIntentSink;
import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.util.RealmLabel;
import io.wispforest.owo.ui.base.BaseComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.*;
import net.minecraft.client.MinecraftClient;
import java.util.*;

/** 两个窗口各持有选择与镜头，最小化不丢状态；每帧只读取权威 Store 快照。 */
public final class BodyModelContent implements AutoCloseable {
    private final UiWindowManager.WindowState owner;
    private final BodyInspectComponent body = new BodyInspectComponent();
    private final UiIntentSink<CultivationIntent> intents;
    private final FlowLayout root = Containers.horizontalFlow(Sizing.fill(100), Sizing.fill(100));
    private final FlowLayout stage = Containers.verticalFlow(Sizing.fixed(1), Sizing.fill(100));
    private final Targets targets = new Targets();
    private final Details details = new Details();
    private final Controls controls = new Controls();
    private int layoutWidth, layoutHeight;
    private String feedback = "";
    private boolean overviewExpanded;
    private int detailScroll;

    public BodyModelContent(UiWindowManager.WindowState owner, BodyInspectComponent.Layer layer, UiIntentSink<CultivationIntent> intents) {
        this.owner = owner; this.intents = intents;
        body.setActiveLayer(layer);
        root.padding(Insets.of(8)); root.gap(8);
        var side = Containers.verticalFlow(Sizing.fixed(114), Sizing.fill(100)); side.gap(6);
        side.child(new Summary()); side.child(targets); root.child(side);
        stage.gap(5); stage.child(body); stage.child(details); stage.child(controls); root.child(stage);
        refresh();
    }
    public FlowLayout component() { return root; }
    public BodyInspectComponent body() { return body; }
    public void refresh() { body.setPhysicalBody(PhysicalBodyStore.snapshot()); body.setMeridianBody(MeridianStateStore.snapshot()); }
    public void layout(int w, int h) {
        refresh();
        if (layoutWidth == w && layoutHeight == h) return;
        layoutWidth = w; layoutHeight = h;
        stage.horizontalSizing(Sizing.fixed(Math.max(1, w - 138)));
        int detailHeight = overviewExpanded ? Math.max(42, (h - 64) / 2) : h < 250 ? 28 : 58;
        details.verticalSizing(Sizing.fixed(detailHeight));
        body.verticalSizing(Sizing.fixed(Math.max(1, h - 16 - detailHeight - 48 - 10)));
        targets.verticalSizing(Sizing.fixed(Math.max(1, h - 16 - 56 - 6)));
    }
    public void invalidate() { body.invalidate(); }
    @Override public void close() { body.close(); }
    private boolean meridian() { return body.activeLayer() == BodyInspectComponent.Layer.MERIDIAN; }
    private static void text(OwoUIDrawContext ctx, String value, int x, int y, int w, int color) {
        var tr = MinecraftClient.getInstance().textRenderer;
        ctx.drawText(tr, tr.trimToWidth(value, Math.max(1, w)), x, y, color, false);
    }
    private static String number(double value) { return String.format(Locale.ROOT, "%.1f", value); }
    private final class Summary extends BaseComponent {
        Summary() { id("body-summary"); sizing(Sizing.fill(100), Sizing.fixed(56)); }
        @Override public void draw(OwoUIDrawContext ctx, int mx, int my, float partial, float delta) {
            var player = PlayerStateStore.snapshot(); var channels = body.meridianBody();
            text(ctx, meridian() ? "经 脉 · 内 观" : "体 表 · 内 观", x, y, width, 0xFFE9D6AF);
            text(ctx, RealmLabel.displayName(channels == null ? player.realm() : channels.realm()), x, y+16, width, 0xFF99AEB8);
            text(ctx, "真元 " + number(player.spiritQiCurrent()) + " / " + number(player.spiritQiMax()), x, y+31, width, 0xFFC7B281);
            ctx.fill(x, y+48, x+width, y+50, 0xFF33424B);
            ctx.fill(x, y+48, x+(int)(width*player.spiritQiFillRatio()), y+50, 0xFFCCAB6B);
        }
    }
    private final class Targets extends BaseComponent {
        private int first;
        Targets() { id("body-targets"); sizing(Sizing.fill(100), Sizing.fixed(1)); cursorStyle(CursorStyle.HAND); }
        private List<?> entries() {
            if (!meridian()) return Arrays.asList(BodyPart.values());
            var snapshot = body.meridianBody();
            return snapshot == null ? List.of() : Arrays.stream(MeridianChannel.values())
                .filter(ch -> snapshot.channel(ch) != null && body.meridianFilter().includes(ch)).toList();
        }
        @Override public void draw(OwoUIDrawContext ctx, int mx, int my, float partial, float delta) {
            ctx.fill(x, y, x+width, y+height, 0xFF10191F);
            ctx.enableScissor(x, y, x+width, y+height);
            try {
                var entries = entries(); first = Math.min(first, Math.max(0, entries.size()-Math.max(1,height/29)));
                for (int i = first; i < entries.size() && (i-first)*29 < height; i++) {
                    Object entry = entries.get(i); int top = y+(i-first)*29;
                    boolean selected = entry == body.selectedPart() || entry == body.selectedChannel();
                    if (selected) { ctx.fill(x,top,x+width,top+28,0xFF293B45); ctx.fill(x,top+3,x+2,top+25,0xFFD5B776); }
                    String label, state;
                    if (entry instanceof MeridianChannel ch) {
                        var s = body.meridianBody().channel(ch); label = ch.displayName();
                        state = s.blocked() ? "未通 · " + Math.round(s.healProgress()*100) + "%" : s.damage().label();
                        if (body.meridianBody().targetMeridian() == ch) state += " · 冲脉目标";
                    } else {
                        var part = (BodyPart) entry; label = part.displayName();
                        state = body.physicalBody() == null ? "等待同步" : body.physicalBody().part(part).wound().label();
                    }
                    text(ctx,label,x+8,top+4,width-13,selected?0xFFF2DCAB:0xFFCCD5DB);
                    text(ctx,state,x+8,top+17,width-13,0xFF829DAA);
                }
                if (entries.isEmpty()) text(ctx,"等待经脉同步",x+6,y+8,width-12,0xFF839AA6);
            } finally { ctx.disableScissor(); }
        }
        @Override public boolean onMouseDown(double mx, double my, int button) {
            if (button != 0) return false;
            var entries = entries(); int i = first+(int)my/29;
            if (i >= 0 && i < entries.size()) {
                if (entries.get(i) instanceof MeridianChannel ch) body.setSelectedChannel(ch);
                else body.setSelectedPart((BodyPart)entries.get(i));
                feedback = "";
            }
            return true;
        }
        @Override public boolean onMouseScroll(double mx, double my, double amount) {
            first = Math.max(0, Math.min(Math.max(0, entries().size()-Math.max(1,height/29)), first-(int)Math.signum(amount)*3));
            return true;
        }
    }
    private final class Details extends BaseComponent {
        Details() { id("body-details"); sizing(Sizing.fill(100), Sizing.fixed(64)); }
        @Override public void draw(OwoUIDrawContext ctx, int mx, int my, float partial, float delta) {
            List<String> lines = new ArrayList<>();
            if (meridian()) {
                var selected = body.selectedChannel(); var state = body.meridianBody() == null || selected == null ? null : body.meridianBody().channel(selected);
                lines.add(selected == null ? "真元池 · 周流" : selected.displayName());
                if (state != null) {
                    lines.add(state.blocked() ? "冲脉进度  " + Math.round(state.healProgress()*100)+"%" : "流量  "+number(state.currentFlow())+" / "+number(state.capacity()));
                    lines.add("损伤  "+state.damage().label()+"     污染  "+Math.round(state.contamination()*100)+"%");
                    lines.add("裂痕  "+body.meridianBody().cracksFor(selected)+"    原色流出 · 青色回流");
                } else { lines.add("真元原色流出 · 青色回流"); lines.add("选择经脉，镜头将停在它的流路上"); }
            } else {
                var selected = body.selectedPart(); var state = body.physicalBody() == null || selected == null ? null : body.physicalBody().part(selected);
                lines.add(selected == null ? "全身 · 体表" : selected.displayName());
                if (state != null) {
                    lines.add("伤势  "+state.wound().label()+"     夹板  "+(state.splinted()?"已固定":"无"));
                    lines.add("出血  "+(state.bleedRate()>0?"持续":"无")+"     恢复  "+Math.round(state.healProgress()*100)+"%");
                } else lines.add("选择肢体，查看伤势与恢复状态");
            }
            var applied = meridian() ? body.selectedChannel()==null ? null : body.meridianItemAt(body.selectedChannel())
                : body.selectedPart()==null ? null : body.physicalItemAt(body.selectedPart());
            if(applied!=null) lines.add("外敷  "+applied.displayName());
            if (overviewExpanded) {
                var snapshot = body.meridianBody();
                if (snapshot != null) {
                    if (snapshot.hasLifespanPreview()) {
                        lines.add("寿元  "+number(snapshot.yearsLived())+" / "+snapshot.lifespanCapByRealm()
                            +"    余 "+number(snapshot.remainingYears())+(snapshot.isWindCandle()?" · 风烛":""));
                        lines.add("死亡折寿  "+snapshot.deathPenaltyYears()+"    流速 ×"+number(snapshot.lifespanTickRateMultiplier()));
                    }
                    lines.add("污染总量  "+number(snapshot.contaminationTotal()));
                    lines.add("真元色  "+snapshot.qiColorMain().label()
                        +(snapshot.qiColorSecondary()==null?"":" / "+snapshot.qiColorSecondary().label())
                        +(snapshot.qiColorChaotic()?" · 杂":"")+(snapshot.qiColorHunyuan()?" · 混元":""));
                    snapshot.qiColorPracticeWeights().forEach((color,weight) -> lines.add("  "+color.label()+"  "+number(weight)));
                    lines.add(QiColorVectorHud.hunyuanDistanceText(snapshot));
                    String quota = StatusPanelExtension.ascensionQuotaLine(snapshot.realm());
                    if (!quota.isEmpty()) lines.add(quota);
                    snapshot.activeEffects().forEach(effect -> lines.add(effect.name()+"  "+effect.description()));
                }
                var observation = QiColorObservedStore.snapshot();
                String observed = observation == null ? "" : observation.displayText();
                if (!observed.isEmpty()) lines.add(observed);
                lines.addAll(com.bong.client.inventory.InspectScreen.swordBondInfoLines(
                    com.bong.client.hud.SwordBondHudStateStore.snapshot()));
                lines.addAll(CultivationOverview.describe(PlayerStateStore.snapshot()).lines());
            }
            if (!feedback.isEmpty()) { if (lines.size()>1) lines.set(1,feedback); else lines.add(feedback); }
            detailScroll = Math.min(detailScroll, Math.max(0, lines.size()-Math.max(1,height/14)));
            for (int i=detailScroll; i<lines.size() && (i-detailScroll)*14+10<=height; i++)
                text(ctx,lines.get(i),x+3,y+(i-detailScroll)*14+2,width-6,i==0?0xFFE7C993:0xFFA6B9C3);
        }
        @Override public boolean onMouseScroll(double mx,double my,double amount) {
            detailScroll = Math.max(0, detailScroll-(int)Math.signum(amount)*2);
            return true;
        }
    }
    private final class Controls extends BaseComponent {
        Controls() { id("body-controls"); sizing(Sizing.fill(100),Sizing.fixed(48)); cursorStyle(CursorStyle.HAND); }
        @Override public void draw(OwoUIDrawContext ctx,int mx,int my,float partial,float delta) {
            String[] top = {"全身",body.material()?"灰模":"皮肤",meridian()?body.meridianFilter().label():"经脉",overviewExpanded?"收起":"详情"};
            for(int i=0;i<top.length;i++) {
                int left=x+width*i/top.length; text(ctx,top[i],left+4,y+5,width/top.length-8,0xFFD3C5A4);
                ctx.fill(left+3,y+18,x+width*(i+1)/top.length-5,y+19,0xFF4B606B);
            }
            String[] bottom = meridian()?new String[]{"设目标","突破","渡虚","流速","容量"}:new String[]{"拖拽旋转 · 滚轮缩放"};
            for(int i=0;i<bottom.length;i++) text(ctx,bottom[i],x+width*i/bottom.length+3,y+31,width/bottom.length-5,0xFF98B3BF);
        }
        @Override public boolean onMouseDown(double mx,double my,int button) {
            if(button!=0 || owner.closed()) return false;
            if(my<23) {
                int i=Math.min(3,(int)(mx*4/Math.max(1,width)));
                if(i==0) body.overview();
                else if(i==1) body.material(!body.material());
                else if(i==3) {
                    overviewExpanded = !overviewExpanded; detailScroll=0;
                    int w=layoutWidth,h=layoutHeight; layoutWidth=0; layout(w,h);
                } else if(meridian()) {
                    var all=BodyInspectComponent.MeridianFilter.values(); body.setMeridianFilter(all[(body.meridianFilter().ordinal()+1)%all.length]);
                    feedback="筛选："+body.meridianFilter().label();
                } else com.bong.client.ui.window.UiWindowRuntime.openBody(BodyInspectComponent.Layer.MERIDIAN);
            } else if(meridian()) {
                var actions=CultivationIntent.Action.values(); int i=Math.min(actions.length-1,(int)(mx*actions.length/Math.max(1,width)));
                var result=intents.dispatch(new CultivationIntent(actions[i],body.selectedChannel()));
                feedback=result.kind()==com.bong.client.ui.intent.UiIntentResult.Kind.LOCAL_ACCEPTED ? "请求已送出，等待回应" : result.reason();
            }
            return true;
        }
    }
}
