package com.bong.client.inventory.component;

import com.bong.client.inventory.model.*;
import com.bong.client.inventory.state.PlayerRaceIdentityStore;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.ui.model.BodyModelGeometry;
import com.bong.client.ui.model.BodyModelCapture;
import com.bong.client.ui.model.ModelOverlayMesh;
import com.bong.client.ui.model.ModelPreviewComponent;
import io.wispforest.owo.ui.core.OwoUIDrawContext;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.*;
import net.minecraft.client.render.entity.PlayerEntityRenderer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.math.Box;
import net.minecraft.util.math.Vec3d;
import org.joml.Matrix4f;
import org.joml.Vector3f;

import java.util.*;
import java.util.function.Consumer;

/** 自身内观与通用模型预览共享镜头、网格取景和输入；领域选择只在点击释放后提交。 */
public final class BodyInspectComponent extends ModelPreviewComponent {
    public enum Layer { PHYSICAL, MERIDIAN }
    public enum MeridianFilter {
        ALL("全部"), ARM("手经"), LEG("足经"), EXTRA("奇经");
        private final String label;
        MeridianFilter(String label) { this.label = label; }
        public String label() { return label; }
        public boolean includes(MeridianChannel ch) {
            return switch (this) {
                case ALL -> true;
                case ARM -> ch.region() == MeridianChannel.BodyRegion.LEFT_ARM || ch.region() == MeridianChannel.BodyRegion.RIGHT_ARM;
                case LEG -> ch.region() == MeridianChannel.BodyRegion.LEFT_LEG || ch.region() == MeridianChannel.BodyRegion.RIGHT_LEG;
                case EXTRA -> ch.family() == MeridianChannel.Family.EXTRAORDINARY;
            };
        }
    }
    private Layer layer = Layer.PHYSICAL;
    private MeridianFilter filter = MeridianFilter.ALL;
    private PhysicalBody physical;
    private MeridianBody meridians;
    private BodyPart selectedPart = BodyPart.CHEST, hoveredPart, highlightedPart;
    private MeridianChannel selectedChannel, hoveredChannel, highlightedChannel;
    private final Set<MeridianChannel> techniques = EnumSet.noneOf(MeridianChannel.class);
    private final List<Consumer<MeridianChannel>> listeners = new ArrayList<>();
    private final Map<BodyPart, InventoryItem> physicalApplied = new EnumMap<>(BodyPart.class);
    private final Map<MeridianChannel, InventoryItem> meridianApplied = new EnumMap<>(MeridianChannel.class);
    private BodyModelGeometry geometry;
    private boolean entrance = true, focusPending = true, pressed;
    private Box modelBounds;
    private double dragged, seconds;
    private Matrix4f projection;

    public BodyInspectComponent() {
        id("body-model"); material(false); camera().autoRotate(false);
    }
    public void setPhysicalBody(PhysicalBody body) {
        if (physical == body) return;
        physical = body; physicalApplied.clear();
        if (body != null) physicalApplied.putAll(body.allAppliedItems());
    }
    public void setMeridianBody(MeridianBody body) {
        if (meridians == body) return;
        meridians = body; meridianApplied.clear();
        if (body != null) meridianApplied.putAll(body.allAppliedItems());
        if (selectedChannel != null && (body == null || body.channel(selectedChannel) == null)) setSelectedChannel(null);
    }
    public PhysicalBody physicalBody() { return physical; }
    public MeridianBody meridianBody() { return meridians; }
    public Layer activeLayer() { return layer; }
    public void setActiveLayer(Layer value) { layer = value; entrance = true; }
    public MeridianFilter meridianFilter() { return filter; }
    public void setMeridianFilter(MeridianFilter value) {
        filter = value;
        if (selectedChannel != null && !value.includes(selectedChannel)) setSelectedChannel(null);
    }
    public BodyPart selectedPart() { return selectedPart; }
    public void setSelectedPart(BodyPart part) { selectedPart = part; focus(false); }
    public MeridianChannel selectedChannel() { return selectedChannel; }
    public void setSelectedChannel(MeridianChannel channel) {
        if (selectedChannel == channel) return;
        selectedChannel = channel; focus(false);
        listeners.forEach(listener -> listener.accept(channel));
    }
    public void addSelectionListener(Consumer<MeridianChannel> listener) { listeners.add(listener); }
    public MeridianChannel focusedChannel() { return hoveredChannel != null ? hoveredChannel : selectedChannel; }
    public BodyPart hoveredPart() { return hoveredPart; }
    public MeridianChannel hoveredChannel() { return hoveredChannel; }
    public void applyPhysicalItem(BodyPart part, InventoryItem item) { physicalApplied.put(part, item); }
    public InventoryItem removePhysicalItem(BodyPart part) { return physicalApplied.remove(part); }
    public InventoryItem physicalItemAt(BodyPart part) { return physicalApplied.get(part); }
    public void applyMeridianItem(MeridianChannel ch, InventoryItem item) { meridianApplied.put(ch, item); }
    public InventoryItem removeMeridianItem(MeridianChannel ch) { return meridianApplied.remove(ch); }
    public InventoryItem meridianItemAt(MeridianChannel ch) { return meridianApplied.get(ch); }
    public void setPhysicalHighlight(BodyPart part, boolean valid) { highlightedPart = valid ? part : null; }
    public void setMeridianHighlight(MeridianChannel ch, boolean valid) { highlightedChannel = valid ? ch : null; }
    public void clearHighlight() { highlightedPart = null; highlightedChannel = null; }
    public void setTechniqueMeridianHighlights(Collection<MeridianChannel> channels) {
        techniques.clear();
        if (channels != null) channels.stream().filter(Objects::nonNull).forEach(techniques::add);
    }
    public void clearTechniqueMeridianHighlights() { techniques.clear(); }
    public Set<MeridianChannel> techniqueMeridianHighlightsForTests() { return Set.copyOf(techniques); }

    public boolean hasModelGeometry() { return geometry != null; }

    public boolean anatomical() {
        return PlayerRaceIdentityStore.formIsHumanoid() && PlayerRaceIdentityStore.intrinsicIsHumanoid();
    }
    public void overview() {
        selectedChannel = null; selectedPart = null;
        listeners.forEach(listener -> listener.accept(null));
        camera().focus(new Vec3d(.5, .5, .5), 1, 8, -5, false);
    }
    private void focus(boolean intro) {
        if (modelBounds == null || geometry == null) { focusPending = true; return; }
        Vec3d target; float zoom;
        if (layer == Layer.MERIDIAN && selectedChannel != null) {
            var box = BodyModelGeometry.bounds(geometry.route(selectedChannel));
            target = box.getCenter();
            zoom = (float) Math.max(1.15, Math.min(2.35, 1.55 / Math.max(.6, box.getYLength())));
        } else if (layer == Layer.PHYSICAL && selectedPart != null) {
            target = geometry.part(selectedPart).center(); zoom = intro ? 1.35f : 2.4f;
        } else { target = geometry.pool(); zoom = 1.25f; }
        camera().focus(BodyModelGeometry.normalized(target, modelBounds), zoom,
            selectedChannel == MeridianChannel.DU ? 172 : 8, -5, intro);
    }
    @Override public void invalidate() { super.invalidate(); geometry = null; projection = null; modelBounds = null; entrance = true; focusPending = true; }
    @Override protected void collectModel(MatrixStack matrices, VertexConsumerProvider vertices, float partialTicks) {
        geometry = null;
        var client = MinecraftClient.getInstance();
        var renderer = client.getEntityRenderDispatcher().getRenderer(client.player);
        if (anatomical() && renderer instanceof PlayerEntityRenderer playerRenderer) {
            // 真实玩家 renderer 负责皮肤、粗细手臂和当前姿态；在同一次绘制中捕获最终部件变换。
            geometry = BodyModelCapture.collect(playerRenderer.getModel(),
                () -> super.collectModel(matrices, vertices, partialTicks));
        } else super.collectModel(matrices, vertices, partialTicks);
    }
    @Override protected void prepareCamera(Box bounds) {
        modelBounds = bounds;
        if (geometry != null && (entrance || focusPending)) {
            focus(entrance); entrance = false; focusPending = false;
        }
    }
    @Override public void draw(OwoUIDrawContext ctx, int mouseX, int mouseY, float partialTicks, float delta) {
        super.draw(ctx, mouseX, mouseY, partialTicks, delta);
        hoveredPart = layer == Layer.PHYSICAL ? bodyPartAtScreen(mouseX, mouseY) : null;
        hoveredChannel = layer == Layer.MERIDIAN ? channelAtScreen(mouseX, mouseY) : null;
        if (!hasModelGeometry()) ctx.drawText(MinecraftClient.getInstance().textRenderer,
            "当前形态尚无三维内观定位", x + 8, y + 8, 0xFFB9C7CB, false);
    }
    @Override protected void drawModelOverlay(OwoUIDrawContext ctx, MatrixStack matrices, Box bounds,
                                               int mouseX, int mouseY, float elapsed) {
        projection = new Matrix4f(matrices.peek().getPositionMatrix()); seconds += elapsed;
        if (!anatomical() || geometry == null) return;
        var buffers = MinecraftClient.getInstance().getBufferBuilders().getEntityVertexConsumers();
        var buffer = buffers.getBuffer(RenderLayer.getGuiOverlay());
        try {
            if (layer == Layer.PHYSICAL) drawWounds(buffer, matrices);
            else if (meridians != null) drawMeridians(buffer, matrices);
        } catch (RuntimeException | Error original) {
            try { buffers.draw(RenderLayer.getGuiOverlay()); }
            catch (RuntimeException | Error cleanup) { if (cleanup != original) original.addSuppressed(cleanup); }
            throw original;
        }
        buffers.draw(RenderLayer.getGuiOverlay());
    }
    private void drawWounds(VertexConsumer buffer, MatrixStack matrices) {
        for (var part : BodyPart.values()) {
            var state = physical == null ? null : physical.part(part);
            boolean focused = part == selectedPart || part == hoveredPart || part == highlightedPart;
            if (!focused && (state == null || state.wound() == WoundLevel.INTACT)) continue;
            var region = geometry.part(part);
            int color = focused ? 0xFFEBCC8C : state.wound().color();
            ModelOverlayMesh.orb(buffer, matrices, region.anchor(.5, .5, 1), focused ? .026 : .018, color);
            for (var edge : region.edges()) for (int i = 1; i < edge.size(); i++)
                ModelOverlayMesh.tube(buffer, matrices, edge.get(i - 1), edge.get(i), .003, alpha(color, .65));
        }
    }
    private void drawMeridians(VertexConsumer buffer, MatrixStack matrices) {
        double qi = PlayerStateStore.snapshot().spiritQiFillRatio();
        int hue = meridians.qiColorMain().argb();
        for (var ch : meridians.allChannels().keySet()) if (filter.includes(ch)) {
            var state = meridians.channel(ch);
            boolean chosen = ch == selectedChannel || ch == hoveredChannel || ch == highlightedChannel || techniques.contains(ch);
            double strength = selectedChannel == null ? .65 : chosen ? 1 : .16;
            var path = geometry.route(ch);
            var returningPath = geometry.returnRoute(ch);
            boolean flows = carriesQi(state, qi);
            int color = alpha(flows ? hue : 0xFF65727F, strength * (flows ? .85 : .45));
            for (int i = 1; i < path.size(); i++) {
                var a = path.get(i - 1); var b = path.get(i);
                if (state.damage() == ChannelState.DamageLevel.SEVERED && Math.abs(i - path.size() / 2) < 3) continue;
                ModelOverlayMesh.tube(buffer, matrices, a, b, chosen ? .006 : .003, color);
                if (flows) ModelOverlayMesh.tube(buffer, matrices, returningPath.get(i-1), returningPath.get(i), .0025,
                    alpha(0xFF75C7CE, strength * .6));
            }
            if (!flows) continue;
            double speed = .18 + .36 * Math.min(1, state.effectiveFlow() / Math.max(1, state.capacity()));
            for (int pulse = 0; pulse < 3; pulse++) {
                double t = (seconds * speed + pulse / 3.0) % 1;
                var outward = BodyModelGeometry.sample(path, t);
                var returning = BodyModelGeometry.sample(returningPath, t);
                int pulseColor = state.contamination() > 0 && t < state.contamination() ? 0xFFC68BCE : 0xFFFFE1A4;
                ModelOverlayMesh.orb(buffer, matrices, outward, chosen ? .014 : .010, alpha(pulseColor, strength));
                ModelOverlayMesh.orb(buffer, matrices, returning, .008, alpha(0xFF9AE8E4, strength));
            }
        }
        double pulse = qi > 0 ? 1 + .07 * Math.sin(seconds * Math.PI * 2) : 1;
        ModelOverlayMesh.orb(buffer, matrices, geometry.pool(), .09 * pulse, alpha(hue, .16));
        ModelOverlayMesh.orb(buffer, matrices, geometry.pool(), .065 * Math.cbrt(qi) * pulse, alpha(hue, .75));
        ModelOverlayMesh.orb(buffer, matrices, geometry.pool(), .018 * Math.cbrt(qi), 0xFFFFF0C7);
    }
    public static boolean carriesQi(ChannelState state, double poolRatio) {
        return state != null && Double.isFinite(poolRatio) && poolRatio > 0 && state.effectiveFlow() > 0;
    }
    private static int alpha(int color, double opacity) { return color & 0xFFFFFF | (int) (255 * opacity) << 24; }
    private Vec3d project(Vec3d p) {
        var v = projection.transformPosition((float) p.x, (float) p.y, (float) p.z, new Vector3f());
        return new Vec3d(v.x, v.y, v.z);
    }
    private boolean inside(double sx, double sy) { return anatomical() && geometry != null && projection != null && sx >= x && sx < x + width && sy >= y && sy < y + height; }
    public BodyPart bodyPartAtScreen(double sx, double sy) {
        if (!inside(sx, sy)) return null;
        // 从 GUI 近面投射到模型局部坐标；缩放后整块肢体仍可选，不只命中中心小圆。
        var inverse = new Matrix4f(projection).invert();
        var near = inverse.transformPosition((float)sx, (float)sy, 10000, new Vector3f());
        var far = inverse.transformPosition((float)sx, (float)sy, -10000, new Vector3f());
        var start = new Vec3d(near.x, near.y, near.z);
        var end = new Vec3d(far.x, far.y, far.z);
        BodyPart best = null;
        double distance = Double.POSITIVE_INFINITY;
        for (var part : BodyPart.values()) {
            var hit = geometry.part(part).raycast(start, end);
            if (hit.isPresent() && hit.get().squaredDistanceTo(start) < distance) {
                best = part; distance = hit.get().squaredDistanceTo(start);
            }
        }
        return best;
    }
    public MeridianChannel channelAtScreen(double sx, double sy) {
        if (!inside(sx, sy) || meridians == null) return null;
        MeridianChannel best = null; double distance = 6;
        for (var ch : meridians.allChannels().keySet()) if (filter.includes(ch)) {
            var path = geometry.route(ch);
            for (int i = 1; i < path.size(); i++) {
                var a = project(path.get(i - 1)); var b = project(path.get(i));
                double dx = b.x - a.x, dy = b.y - a.y;
                double t = Math.max(0, Math.min(1, ((sx-a.x)*dx+(sy-a.y)*dy) / Math.max(.00001, dx*dx+dy*dy)));
                double d = Math.hypot(sx - a.x - t*dx, sy - a.y - t*dy);
                if (d < distance) { best = ch; distance = d; }
            }
        }
        return best;
    }
    public boolean clickSelectMeridian(double sx, double sy) {
        var ch = channelAtScreen(sx, sy);
        if (ch == null || layer != Layer.MERIDIAN) return false;
        setSelectedChannel(ch); return true;
    }
    @Override public boolean onMouseDown(double mx, double my, int button) {
        pressed = button == 0; dragged = 0;
        return super.onMouseDown(mx, my, button);
    }
    @Override public boolean onMouseDrag(double mx, double my, double dx, double dy, int button) {
        dragged += Math.hypot(dx, dy); return super.onMouseDrag(mx, my, dx, dy, button);
    }
    @Override public boolean onMouseUp(double mx, double my, int button) {
        if (pressed && button == 0 && dragged < 4) {
            if (layer == Layer.MERIDIAN) clickSelectMeridian(x + mx, y + my);
            else { var part = bodyPartAtScreen(x + mx, y + my); if (part != null) setSelectedPart(part); }
        }
        pressed = false; return super.onMouseUp(mx, my, button);
    }
    @Override public void close() { super.close(); geometry = null; projection = null; listeners.clear(); }
}
