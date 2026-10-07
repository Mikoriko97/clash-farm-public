@tool
class_name FloraKind
extends Resource
## Один вид рослинності (docs/MAINLAND.md §9): який шматок паку, який слід
## займає, де росте і як густо. Дані правляться в редакторі
## (`scenes/mainland/flora/*.tres`, `scenes/island/wild/*.tres`); розкидає їх
## `Scatter` (синій шум з радіусами — ніщо не росте одне в одному). Материк
## запікається наперед (`tools/bake_flora.gd`), острів розкидається на льоту.
## Порядок у масиві — це шари: спершу крони (якорі), потім підлісок і трави.

## Шматок паку рослинності (GLB) або сцена з MeshInstance3D усередині.
@export var scene: PackedScene
## Шар для пензля зон (`addons/flora_paint`, маска `assets/mainland/flora_paint.png`):
## канал R — крони, G — кущі, B — покрив (трави, квіти), A — деталі (каміння, колоди, гриби).
@export_enum("canopy", "shrub", "ground", "detail") var layer: String = "ground"
## Радіус сліду (м) при масштабі 1: дві точки не бувають ближче за суму радіусів.
@export_range(0.05, 8.0, 0.01) var radius_m: float = 1.0
## Крок сітки кандидатів (м): найбільша густота ≈ 1 / крок². Зазвичай ≈ 2 радіуси.
@export_range(0.5, 40.0, 0.1) var spacing_m: float = 4.0
## Частка кандидатів, що приживається там, де все інше дозволяє.
@export_range(0.0, 1.0) var density: float = 0.5
## Канал карти біомів, який дає вагу: луки (R), ліс (G), скелі (B), пісок (A).
@export_enum("meadow", "forest", "rock", "sand", "any") var biome: String = "forest"
## Середина мʼякого порогу ваги біому (±0.15): узлісся рідшає поступово.
@export_range(0.0, 1.0) var biome_min: float = 0.5
## Кластери: поріг шуму (−1…1). −1 — рівномірно; вище — рідше, зате купками
## (гаї, галявини, плями квітів).
@export_range(-1.0, 1.0) var cluster: float = -1.0
## Масштаб шуму кластерів: 0.01 ≈ плями по 100 м, 0.06 ≈ по 15 м.
@export_range(0.002, 0.2, 0.001) var cluster_freq: float = 0.02
## Найкрутіший схил (градуси), на якому вид ще стоїть.
@export_range(0.0, 90.0) var slope_max_deg: float = 30.0
## Найнижча висота над рівнем моря (м): вода — 0.3.
@export var above_sea_m: float = 0.6
## Лише в цій відстані (м) від води — очерет, прибережна трава; 0 — будь-де.
@export var near_water_m: float = 0.0
## Лише під кронами (не далі стільки метрів від дерева-якоря) — папороть,
## гриби, лісові кущі; 0 — будь-де.
@export var under_canopy_m: float = 0.0
## Цей вид сам є кроною-якорем для наступних шарів.
@export var anchor: bool = false
## Літери зон материка, де вид росте (`shared/mainland_zones.json`): порожньо —
## усюди на суші, крім міста (C); "C" дозволяє місто, "S" — болото тощо.
@export var zones: String = ""
## Вид вузла ресурсу сервера, яким стає кожен екземпляр (tree_pine, rock,
## herb…): такі точки йдуть у `shared/mainland_nodes.json`, а не в декор.
@export var node_kind: String = ""
@export var scale_min: float = 0.85
@export var scale_max: float = 1.2
## Нахилятися за схилом (камені, колоди) чи стояти вертикально.
@export var tilt: bool = false
## Наскільки втопити основу в ґрунт (м), щоб край не висів на схилі.
@export var sink_m: float = 0.05
## Дальність видимості (м); 0 — завжди. Дрібне зникає здалеку і не коштує кадрів.
@export var visible_m: float = 0.0
## Показувати і в далеких чанках (LOD 1) — дерева, валуни; дрібне лише поруч.
@export var far_chunks: bool = false
## Кидати тінь (дрібне — ні: дешевше, а різниці не видно).
@export var shadows: bool = true
## Відтінок листя екземпляра (множиться в шейдері листя Simple Trees): між `tint_min`
## і `tint_max` плямами по місцевості з невеликим розкидом; білий — без відтінку.
@export var tint_min: Color = Color.WHITE
@export var tint_max: Color = Color.WHITE
## Частка екземплярів з осіннім листям.
@export_range(0.0, 1.0) var autumn_chance: float = 0.0
@export var autumn_tint: Color = Color(1.0, 0.78, 0.42)
## Радіус циліндра-колайдера (м, при масштабі 1): стовбур, кущ, валун. 0 —
## персонаж проходить наскрізь (трави, квіти, гриби).
@export var collider_radius_m: float = 0.0
@export var collider_height_m: float = 2.0
