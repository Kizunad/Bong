#!/usr/bin/env python3
"""Batch-generate item icons for all missing ItemTemplates."""
import subprocess, sys, os, json, time
from pathlib import Path

MAX_RETRIES = 3
RETRY_DELAY = 15  # seconds

GEN_PY = Path(__file__).parent / "gen.py"
OUT_DIR = Path(__file__).parent.parent.parent / "local_images"
EXISTING = set()
TEX_DIR = Path(__file__).parent.parent.parent / "client/src/main/resources/assets/bong/textures/item"
if TEX_DIR.is_dir():
    for p in TEX_DIR.iterdir():
        EXISTING.add(p.stem if p.is_file() else p.name)

ITEMS = [
    # ── 基础材料 ──
    ("rough_cloth", "粗布",
     "a piece of coarse grey-brown woven cloth, rough spider-silk fabric, frayed edges, folded once, solid black background"),
    ("tanned_hide", "熟皮",
     "a piece of tanned animal hide, dark brown leather with visible grain texture, slightly curled edges, solid black background"),
    ("bone_chip_mat", "骨片",
     "several thin pale bone shards chipped from beast bones, ivory white with slight yellow tinge, sharp edges, solid black background"),
    ("bone_meal_mat", "骨粉",
     "a small pile of ground bone powder in a crude cloth wrap, off-white powder with tiny bone fragments, solid black background"),
    ("spirit_charcoal", "灵木炭",
     "a chunk of spirit-wood charcoal, dark black carbon block with faint blue-green spiritual veins glowing inside, solid black background"),
    ("rat_tail_oil", "鼠尾油",
     "a small ceramic vial of rat-tail oil, dark amber oily liquid visible through a cracked stopper, greasy residue on the neck, solid black background"),
    ("salt_crystal", "盐蓬晶",
     "a cluster of translucent salt crystals with faint white-blue shimmer, irregular faceted surfaces, solid black background"),
    ("spider_silk_cord", "蛛丝绳",
     "a coil of thin silvery-white spider silk rope, tightly braided, slightly luminous, solid black background"),
    ("wood_plank", "木板",
     "a rough-cut wooden plank, grey-brown aged wood with visible grain and a few knots, solid black background"),
    ("dried_grass", "干草",
     "a bundle of dried yellow-brown wild grass tied with a thin cord, wispy and brittle, solid black background"),
    ("zhenfa_blank_array", "阵法白纸",
     "a piece of ritual array paper, pale yellow-white cloth soaked in cinnabar oil, faint red-orange stain pattern, solid black background"),
    ("powder_dan_sha", "丹砂粉",
     "a small mound of cinnabar powder, deep red-orange fine powder on a cloth wrap, solid black background"),
    ("powder_zhu_sha", "朱砂粉",
     "a small mound of vermilion powder, bright crimson-red fine powder with faint spiritual shimmer, solid black background"),
    ("herb_bundle", "灵草束",
     "a bundle of spirit herbs tied with cord, mixed green and grey-green dried leaves and stems, faint glow at the binding, solid black background"),

    # ── 容器与装具 ──
    ("herb_vial", "药瓶",
     "a small ceramic medicine vial with a spirit-crystal chip stopper, dark clay body, narrow neck, solid black background"),
    ("sealed_vial", "密封药瓶",
     "a sealed medicine vial wrapped tightly with spider-silk cord, ceramic body with faint spiritual seal marks, solid black background"),
    ("herb_pouch", "灵草囊",
     "a coarse cloth herb pouch, dark grey-brown fabric bag with drawstring closure, slightly bulging, solid black background"),
    ("coin_box", "骨币匣",
     "a small wooden box for bone coins, dark wood with cinnabar-lined interior barely visible, iron clasp, solid black background"),
    ("projectile_bag", "暗器袋",
     "a leather hidden-weapon pouch, dark brown animal hide with reinforced lining, multiple compartments, solid black background"),
    ("spirit_seal_box", "封灵匣",
     "a small spirit-iron banded sealed box, dark metal bands over dark wood, faint blue glow at the seams, solid black background"),
    ("dead_drop_box", "死信箱",
     "a buried trade box, dark weathered wood reinforced with iron corners, protective array lines carved on the surface, solid black background"),
    ("moisture_guard", "防潮包",
     "a small moisture-absorbing cloth packet, grey-green ash-moss wrapped in coarse fabric, tied shut, solid black background"),
    ("ore_sack", "矿石袋",
     "a reinforced ore sack, dark coarse cloth bag with thick leather bottom panel, heavy and sagging, solid black background"),
    ("water_skin", "水囊",
     "a leather water skin, dark brown animal hide sewn into a flask shape, wooden stopper, carrying cord, solid black background"),
    ("trade_crate", "货箱",
     "a large wooden trade crate, six dark wood panels nailed together with iron nails, solid black background"),
    ("herb_crate", "灵草箱",
     "a wooden herb storage crate, dark wood box lined with cloth interior, ventilation gaps between planks, solid black background"),
    ("sealed_envelope", "密封信封",
     "a sealed coarse-cloth envelope, grey-brown fabric sealed with dark oil at the edge, tamper-evident, solid black background"),

    # ── 工具与装备 ──
    ("stone_pickaxe", "石镐",
     "a crude stone pickaxe, sharp grey stone head bound to a wooden handle with cord, primitive tool, solid black background"),
    ("stone_axe", "石斧",
     "a crude stone axe, wedge-shaped grey stone clamped onto a wooden handle, primitive tool, solid black background"),
    ("stone_hoe", "石锄",
     "a crude stone hoe, flat grey stone bound to a wooden handle, farming tool, solid black background"),
    ("mortar_stone", "研钵",
     "a stone mortar and pestle, grey carved stone bowl with a grinding stone inside, herb residue stains, solid black background"),
    ("heat_gloves", "绝热手套",
     "a pair of heat-resistant gloves, thick dark leather with spider-silk inner lining visible at the cuff, faint blue shimmer, solid black background"),

    # ── 急救与修炼 ──
    ("arm_splint", "夹板·臂",
     "an arm splint made of two thin wood boards bound with cloth strips, medical supply, solid black background"),
    ("leg_splint", "夹板·腿",
     "a leg splint made of longer wood boards bound with cloth strips, medical supply, solid black background"),
    ("bandage", "止血绷带",
     "a roll of coarse cloth bandage strips, off-white fabric torn into strips, partially rolled, solid white background"),
    ("meridian_salve", "养经膏",
     "a small jar of meridian-nourishing salve, dark ceramic pot with green-grey herbal paste visible inside, solid black background"),
    ("anti_gu_powder", "解蛊散",
     "a paper packet of anti-venom powder, yellow-brown powder with orange tint spilling from torn paper wrapping, solid black background"),
    ("qingzhuo_powder", "清浊散",
     "a paper packet of purification powder, pale grey-green powder with cinnabar specks, solid black background"),
    ("calming_tea", "安神茶",
     "a small cloth pouch of calming tea leaves, pale yellow-green dried fruit and leaf fragments visible, solid black background"),
    ("meditation_mat", "蒲团",
     "a round meditation cushion, woven from dried grass and coarse cloth layers, flat circular shape, solid black background"),
    ("qi_guide_talisman", "灵气引导符",
     "a spirit-guiding talisman, yellow-brown paper with red cinnabar drawn symbols and array lines, solid black background"),
    ("meridian_rubbing", "经脉图拓片",
     "a meridian chart rubbing, thin cloth with green-blue ink showing meridian pathway diagrams, solid black background"),
    ("ningmai_prep_kit", "凝脉散预制包",
     "a pre-packaged alchemy ingredient kit, small cloth bundle with multiple tied compartments containing powders and dried herbs, solid black background"),
    ("huiyuan_decoction", "回元芷煎汤",
     "a ceramic bowl of herbal decoction, dark brown medicinal liquid with steam rising, solid black background"),
    ("spirit_stone_rack", "灵石架",
     "a small wooden rack with iron pegs for hanging spirit stones, simple dark wood frame, solid black background"),

    # ── 阵法与防御 ──
    ("array_flag_basic", "阵旗·凡",
     "a basic array flag, coarse cloth banner on a wooden pole with cinnabar-drawn array symbols in red, solid black background"),
    ("array_eye_basic", "阵眼·凡",
     "a basic array eye core, small iron disc inlaid with a spirit crystal chip at center, faint blue glow, solid black background"),
    ("trip_wire", "预警绊线",
     "a coil of thin spider-silk trip wire with tiny iron needles attached at intervals, barely visible silver thread, solid black background"),
    ("beast_trap", "困兽圈",
     "a beast trap, dark iron jaw trap with rope attachments, menacing spring-loaded mechanism, solid black background"),
    ("qi_scatter_bead", "散真元珠",
     "a small spiritual disruption bead, translucent grey-white sphere with bone powder and crystal dust swirling inside, faint glow, solid black background"),
    ("gather_array_base", "聚灵阵基座",
     "a spirit-gathering array base, dark stone platform with spirit-iron inlays forming array patterns, crystal chips at corners, faint blue glow, solid black background"),
    ("camouflage_net", "伪装网",
     "a camouflage net, woven coarse cloth threaded with dried spirit-grass, grey-green mottled pattern, solid black background"),

    # ── 基础武器 ──
    ("stone_spearhead", "石矛头",
     "a sharpened stone spearhead, grey flint knapped to a point, solid black background"),
    ("sling_weapon", "弹弓",
     "a crude slingshot, Y-shaped wooden fork with leather pouch and cord, solid black background"),
    ("sling_stone", "飞石",
     "several smooth round sling stones, grey polished river pebbles, solid black background"),
    ("wooden_shield", "木盾",
     "a crude wooden shield, dark wood planks nailed together with iron reinforcement, solid black background"),
    ("bone_shield", "骨盾",
     "a round shield made of woven beast bones tied with rope, pale ivory bone pieces, solid black background"),
    ("iron_sword_blank", "凡铁剑胎",
     "an unfinished iron sword blank, rough dark grey iron blade shape not yet sharpened, no handle wrap, solid black background"),
    ("iron_dagger", "凡铁匕首",
     "a simple iron dagger, short dark grey blade with plain wooden handle, utilitarian design, solid black background"),
    ("stone_knife", "石刃",
     "a crude stone cutting blade, thin grey flint knapped into a rough knife shape, solid black background"),
    ("bone_spike_crude", "粗骨刺",
     "a crude bone spike, pale ivory beast bone roughly sharpened to a point, throwing weapon, solid black background"),
    ("wooden_club", "木棍",
     "a simple wooden club, dark grey-brown spirit-wood staff carved to a blunt weapon, solid black background"),

    # ── 经济 ──
    ("trade_scale", "交易秤",
     "a simple balance scale, iron beam with two wooden pans hung by cord, for weighing bone coins and herbs, solid black background"),
    ("disguise_wrap", "伪装包裹",
     "a disguise wrapping, grey-green ash-moss wrapped cloth bundle designed to mask spiritual aura, solid black background"),
    ("trade_scale_stand", "交易秤台",
     "a trade scale mounted on a wooden stand, iron balance on a dark wood frame, market stall equipment, solid black background"),
    ("niche_repair_kit", "灵龛修补料",
     "a spirit-niche repair kit, a pouch of crushed stone mixed with spirit-iron filings, dark granular material, solid black background"),

    # ── 住所 ──
    ("torch_item", "火把",
     "a torch, wooden handle wrapped with oil-soaked charcoal rags, flame tip glowing orange, solid black background"),
    ("lantern_item", "灯笼",
     "a spirit-crystal lantern, dark iron frame with a faintly glowing blue crystal core inside, hanging ring on top, solid black background"),
    ("door_bolt", "门闩",
     "a door bolt, dark iron bar with wooden mounting plate, simple security hardware, solid black background"),
    ("moisture_base", "防潮地基",
     "a moisture-proof foundation kit, flat dark stone slabs with ash-moss layer, construction material, solid black background"),
    ("simple_bed", "简易床铺",
     "a simple bed frame, rough dark wood planks with dried grass and coarse cloth layered on top as bedding, solid black background"),
    ("window_grate", "窗栅",
     "a window grate, dark iron bars welded into a grid pattern, ventilation and security, solid black background"),
    ("niche_base", "灵龛基座",
     "a spirit-niche pedestal base, dark stone platform with spirit-iron inlays and wood frame, faint spiritual glow at the seams, solid black background"),

    # ── 炼器可放置物 ──
    ("fan_iron_anvil", "凡铁砧台",
     "a tier-one forge station, dark iron anvil mounted on a spirit-wood frame with rough stone base, usable crafting station, solid black background"),

    # ── 矿石 ──
    ("fan_tie", "凡铁矿",
     "a chunk of common iron ore, dark grey metallic rock with rust-brown streaks, rough and heavy, solid black background"),
    ("cu_tie", "粗铁矿",
     "a chunk of crude iron ore, dark brownish-grey rock with visible impurities and metallic glints, solid black background"),
    ("ling_tie", "灵铁",
     "a piece of spirit iron ore, dark metallic stone with faint blue-green spiritual veins running through it, solid black background"),
    ("ling_jing", "灵晶",
     "a spirit crystal shard, translucent pale blue-white crystalline fragment with internal glow, faceted surfaces, solid black background"),
    ("dan_sha", "丹砂",
     "a chunk of raw cinnabar mineral, deep red-orange crystalline rock, solid black background"),
    ("zhu_sha", "朱砂",
     "a chunk of vermilion ore, bright crimson red mineral with faint spiritual shimmer on fracture surfaces, solid black background"),
    ("xiong_huang", "雄黄",
     "a chunk of realgar mineral, yellow-orange crystalline rock with waxy luster, solid black background"),

    # ── core.toml 物品 ──
    ("starter_talisman", "启程护符",
     "a starter protection talisman, small jade-like pendant on a frayed cord, faint warm glow, worn and ancient, solid black background"),
    ("furnace_fantie", "凡铁炉",
     "a basic alchemy furnace, squat dark iron pot-bellied cauldron on stone base, simple design, soot-stained, solid black background"),
    ("furnace_lingtie", "灵铁炉",
     "a spirit-iron alchemy furnace, refined dark metallic cauldron with faint blue spiritual veins, better craftsmanship than basic, solid black background"),
    ("furnace_xitie", "稀铁炉",
     "a rare-iron alchemy furnace, sleek thin-walled dark metallic cauldron with subtle purple-blue spiritual glow, masterwork quality, solid black background"),
    ("mundane_coffin", "凡物棺材",
     "a crude wooden coffin, dark rough planks nailed together, simple and grim, solid black background"),
    ("spirit_niche_stone", "龛石",
     "a spirit niche stone, small cold dark grey stone with barely visible spiritual markings, unassuming, solid black background"),
    ("spirit_treasure_jizhaojing", "寂照镜",
     "an ancient spirit mirror called Ji Zhao Jing, small circular bronze mirror with cracked surface, a ghostly wisp of consciousness visible in the reflection, solid black background"),
    ("healing_herb_bundle", "止创草束",
     "a bundle of healing herbs, green-grey medicinal grass tied with cord, for wound dressing, solid black background"),
    ("ci_she_hao", "刺舌蒿",
     "a sprig of thorny wormwood herb, dark green spiky leaves with tiny barbs, slightly toxic appearance, solid black background"),
    ("ning_mai_cao", "凝脉草",
     "a stalk of meridian-condensing grass, pale blue-green herb with smooth leaves that seem to hold still energy, solid black background"),
    ("hui_yuan_zhi", "回元枝",
     "a small branch of yuan-restoring twig, warm brown bark with sap drops that glow faintly gold, solid black background"),
    ("chi_sui_cao", "赤髓草",
     "a stalk of red-marrow grass, deep crimson-red herb with thick veined leaves, potent and vivid color, solid black background"),
    ("gu_yuan_gen", "固元根",
     "a thick root of yuan-anchoring herb, dark brown gnarled root with dense texture, heavy and grounding, solid black background"),
    ("kong_shou_hen", "空兽痕",
     "a fragment of void-beast trace, pale grey ash-like bone remnant with an otherworldly shimmer, lightweight and eerie, solid black background"),
    ("jie_gu_rui", "解蛊蕊",
     "a small purple flower bud of anti-venom bloom, deep violet petals with luminous center, grows in dark caves, solid black background"),
    ("yang_jing_tai", "养经苔",
     "a patch of meridian-nourishing moss, dark rust-green thin moss layer, scraped from dead-zone borders, solid black background"),
    ("qing_zhuo_cao", "清浊草",
     "a five-petaled purification herb, grey-green leaves with pale veins, grows at boundary zones, solid black background"),
    ("an_shen_guo", "安神果",
     "a calming spirit fruit, pale yellow peach-shaped fruit with smooth skin, gentle and soothing appearance, solid black background"),
    ("shi_mai_gen", "噬脉根",
     "a meridian-devouring root, dark twisted root with visible toxic purple-black veins, dangerous and rare, solid black background"),
    ("ling_yan_shi_zhi", "灵眼石芝",
     "a spirit-eye stone mushroom, small white jade-like fungus cap with luminous glow, extremely rare, solid black background"),
    ("ye_ku_teng", "夜枯藤",
     "a coil of night-wither vine, dark brown-black dried vine that seems to writhe, faintly drains spiritual energy, solid black background"),
    ("hui_jin_tai", "灰烬苔",
     "a small scraping of ash moss, grey-black thin moss that blends with ash-colored stone, medicinal, solid black background"),
    ("zhen_jie_zi", "针芥子",
     "tiny sharp mustard-like seeds, dark brown spicy herb seeds in a small pile, pungent appearance, solid black background"),
    ("shao_hou_man", "烧喉蔓",
     "a glowing cave tendril, dark vine with bioluminescent orange-red tips that reveal its location, solid black background"),
    ("tian_nu_jiao", "天怒椒",
     "a heavenly-wrath pepper, tiny brilliant red-orange chili with spiritual flame aura, extremely rare, solid black background"),
    ("fu_you_hua", "蜉蝣花",
     "an ephemeral mayfly flower, deceptively beautiful pale pink blossom with hidden madness-inducing toxin, solid black background"),
    ("wu_yan_guo", "无言果",
     "a silence fruit, resembles a calming fruit but without ridges, pale yellow smooth surface, permanently mutes whoever eats it, solid black background"),
    ("hei_gu_jun", "黑骨菌",
     "a black-bone fungus, tiny dark mushroom growing from bone crevice, pure black with deadly aura, solid black background"),
    ("fu_chen_cao", "浮尘草",
     "a floating-dust grass, pale wispy herb with visible spore cloud around it, deceptively harmless appearance, solid black background"),
    ("zhong_yan_teng", "终焉藤",
     "a doom vine, dark twisted thorny vine that burns with faint embers, ultimate poison-gu material, solid black background"),
    ("ling_shui", "灵水",
     "a small ceramic flask of spirit water, translucent pale blue liquid visible through the opening, faint spiritual shimmer on the surface, solid black background"),
    ("worn_grass_pouch", "破草包",
     "a worn-out grass pouch, tattered woven grass bag with loose threads and patches, barely holding together, solid black background"),
    ("grass_pouch", "小草包",
     "a small grass-woven pouch, simple woven grass bag with drawstring, basic but functional, solid black background"),
    ("workbench_item", "制作台",
     "a portable crafting workbench, dark spirit-wood tabletop with bone-nail joints, sturdy and rough, solid black background"),
]

DONE_LOG = OUT_DIR / ".batch_done.json"

def load_done():
    if DONE_LOG.exists():
        return set(json.loads(DONE_LOG.read_text()))
    return set()

def save_done(done: set):
    DONE_LOG.write_text(json.dumps(sorted(done), ensure_ascii=False, indent=2))

def main():
    done = load_done()
    total = len(ITEMS)
    skip_existing = "--skip-existing" in sys.argv

    to_gen = []
    for item_id, name, prompt in ITEMS:
        if item_id in done:
            continue
        if skip_existing and item_id in EXISTING:
            done.add(item_id)
            continue
        to_gen.append((item_id, name, prompt))

    print(f"总计 {total} | 已完成 {len(done)} | 待生成 {len(to_gen)}")

    for i, (item_id, name, prompt) in enumerate(to_gen, 1):
        print(f"\n[{i}/{len(to_gen)}] {item_id} ({name})")
        cmd = [
            sys.executable, str(GEN_PY),
            prompt,
            "--name", item_id,
            "--style", "item",
            "--save-prompt",
        ]
        success = False
        for attempt in range(1, MAX_RETRIES + 1):
            result = subprocess.run(cmd, capture_output=True, text=True)
            if result.returncode == 0:
                print(f"  OK → local_images/{item_id}.png")
                done.add(item_id)
                save_done(done)
                success = True
                break
            else:
                stderr = result.stderr.strip()
                is_retryable = any(code in stderr for code in ["503", "429", "502", "530"])
                if is_retryable and attempt < MAX_RETRIES:
                    print(f"  retry {attempt}/{MAX_RETRIES} (等 {RETRY_DELAY}s)...")
                    time.sleep(RETRY_DELAY)
                else:
                    print(f"  FAIL (exit {result.returncode}, attempt {attempt})")
                    if stderr:
                        for line in stderr.split("\n")[-3:]:
                            print(f"    {line}")
        if not success:
            print("  停止批量生成，修复后重新运行即可从断点续跑。")
            sys.exit(1)

    print(f"\n全部完成！共 {len(done)} 张图标。")

if __name__ == "__main__":
    main()
