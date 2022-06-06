// Compiled by ClojureScript 1.10.773 {}
goog.provide('four_nations.model.map.enrichments');
goog.require('cljs.core');
goog.require('four_nations.model.map.biomes');
goog.require('four_nations.model.map.utils');
goog.require('four_nations.model.map.resources');
goog.require('four_nations.general.types');
/**
 * Given a noise map, calculates the average value of all of the cells within that noise map.
 */
four_nations.model.map.enrichments.average_map_value = (function four_nations$model$map$enrichments$average_map_value(noise){
var all_values = cljs.core.map.call(null,new cljs.core.Keyword(null,"value","value",305978217),cljs.core.vals.call(null,noise));
return cljs.core.quot.call(null,cljs.core.apply.call(null,cljs.core._PLUS_,all_values),cljs.core.count.call(null,all_values));
});
/**
 * Determines if a cell should be land, water, or mountain based on the cell value's relation to
 * the average value of the game map.
 */
four_nations.model.map.enrichments.determine_cell_terrain_type = (function four_nations$model$map$enrichments$determine_cell_terrain_type(cell_value,average_value){
if((cell_value < average_value)){
return new cljs.core.Keyword(null,"water","water",-824098213);
} else {
return new cljs.core.Keyword(null,"land","land",-1402569867);
}
});
/**
 * Determines if water should spread to a particular tile, based on the terrain types of the
 * neighbors and a random chance.
 */
four_nations.model.map.enrichments.water_should_spread_to_tile_QMARK_ = (function four_nations$model$map$enrichments$water_should_spread_to_tile_QMARK_(tile,m,dimensions,spread_chance){
var and__4115__auto__ = four_nations.model.map.utils.some_neighbors_are_QMARK_.call(null,cljs.core.partial.call(null,four_nations.model.map.utils.has_terrain_type_QMARK_,new cljs.core.Keyword(null,"water","water",-824098213)),tile,m,dimensions);
if(cljs.core.truth_(and__4115__auto__)){
return ((four_nations.model.map.utils.has_terrain_type_QMARK_.call(null,new cljs.core.Keyword(null,"land","land",-1402569867),tile)) && ((cljs.core.rand.call(null) < spread_chance)));
} else {
return and__4115__auto__;
}
});
/**
 * Given a game map and spread chance, for every land tile within the game that is near water, the
 * spread chance determines how likely it is that the water will spread to that tile.
 */
four_nations.model.map.enrichments.spread_water = (function four_nations$model$map$enrichments$spread_water(m,dimensions,p__39717){
var map__39718 = p__39717;
var map__39718__$1 = (((((!((map__39718 == null))))?(((((map__39718.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39718.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39718):map__39718);
var water_spread_chance = cljs.core.get.call(null,map__39718__$1,new cljs.core.Keyword(null,"water-spread-chance","water-spread-chance",-136332362));
return four_nations.model.map.utils.map_over_tiles.call(null,(function (tile){
if(cljs.core.truth_(four_nations.model.map.enrichments.water_should_spread_to_tile_QMARK_.call(null,tile,m,dimensions,water_spread_chance))){
return cljs.core.assoc_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111)], null),new cljs.core.Keyword(null,"water","water",-824098213));
} else {
return tile;
}
}),m,dimensions);
});
/**
 * Given a tile, a map, and its dimensions, determines if that tile should be drenched with water,
 * which happens when a land tile is surrounded by water tiles.
 */
four_nations.model.map.enrichments.tile_should_be_drenched_QMARK_ = (function four_nations$model$map$enrichments$tile_should_be_drenched_QMARK_(tile,m,dimensions){
var and__4115__auto__ = four_nations.model.map.utils.all_neighbors_are_QMARK_.call(null,cljs.core.partial.call(null,four_nations.model.map.utils.has_terrain_type_QMARK_,new cljs.core.Keyword(null,"water","water",-824098213)),tile,m,dimensions);
if(cljs.core.truth_(and__4115__auto__)){
return four_nations.model.map.utils.has_terrain_type_QMARK_.call(null,new cljs.core.Keyword(null,"land","land",-1402569867),tile);
} else {
return and__4115__auto__;
}
});
/**
 * Given a game map, finds any land tiles where all of its cardinal neighbors are water and sets
 * those tiles to also be water. This prevents tiny, nonsensical islands.
 */
four_nations.model.map.enrichments.drench_surrounded_land = (function four_nations$model$map$enrichments$drench_surrounded_land(m,dimensions){
return four_nations.model.map.utils.map_over_tiles.call(null,(function (tile){
if(cljs.core.truth_(four_nations.model.map.enrichments.tile_should_be_drenched_QMARK_.call(null,tile,m,dimensions))){
return cljs.core.assoc_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111)], null),new cljs.core.Keyword(null,"water","water",-824098213));
} else {
return tile;
}
}),m,dimensions);
});
/**
 * Returns true if the specified tile should be coastline.
 */
four_nations.model.map.enrichments.should_be_coastline_QMARK_ = (function four_nations$model$map$enrichments$should_be_coastline_QMARK_(tile,m,dimensions){
if(four_nations.model.map.utils.has_terrain_type_QMARK_.call(null,new cljs.core.Keyword(null,"land","land",-1402569867),tile)){
return four_nations.model.map.utils.some_neighbors_are_QMARK_.call(null,cljs.core.partial.call(null,four_nations.model.map.utils.has_terrain_type_QMARK_,new cljs.core.Keyword(null,"water","water",-824098213)),tile,m,dimensions);
} else {
return false;
}
});
/**
 * Given a map, changes the terrain type of any land tiles that are near water to be coastline.
 */
four_nations.model.map.enrichments.add_coastline = (function four_nations$model$map$enrichments$add_coastline(m,dimensions){
return four_nations.model.map.utils.map_over_tiles.call(null,(function (tile){
if(cljs.core.truth_(four_nations.model.map.enrichments.should_be_coastline_QMARK_.call(null,tile,m,dimensions))){
return cljs.core.assoc_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111)], null),new cljs.core.Keyword(null,"coast","coast",1578056704));
} else {
return tile;
}
}),m,dimensions);
});
/**
 * Adds a water border, where all tiles along the edge of the map become water, of a specified size
 * to the map.
 */
four_nations.model.map.enrichments.add_water_border = (function four_nations$model$map$enrichments$add_water_border(m,dimensions,p__39720){
var map__39721 = p__39720;
var map__39721__$1 = (((((!((map__39721 == null))))?(((((map__39721.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39721.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39721):map__39721);
var water_border = cljs.core.get.call(null,map__39721__$1,new cljs.core.Keyword(null,"water-border","water-border",23035014));
var water_width_edge = (new cljs.core.Keyword(null,"width","width",-384071477).cljs$core$IFn$_invoke$arity$1(dimensions) - water_border);
var water_height_edge = (new cljs.core.Keyword(null,"height","height",1025178622).cljs$core$IFn$_invoke$arity$1(dimensions) - water_border);
return cljs.core.map.call(null,(function (p__39723){
var vec__39724 = p__39723;
var map__39727 = cljs.core.nth.call(null,vec__39724,(0),null);
var map__39727__$1 = (((((!((map__39727 == null))))?(((((map__39727.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39727.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39727):map__39727);
var point = map__39727__$1;
var x = cljs.core.get.call(null,map__39727__$1,new cljs.core.Keyword(null,"x","x",2099068185));
var y = cljs.core.get.call(null,map__39727__$1,new cljs.core.Keyword(null,"y","y",-1757859776));
var tile = cljs.core.nth.call(null,vec__39724,(1),null);
return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [point,(((((x < water_border)) || ((x >= water_width_edge)) || ((y < water_border)) || ((y >= water_height_edge))))?cljs.core.assoc_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111)], null),new cljs.core.Keyword(null,"water","water",-824098213)):tile)], null);
}),m);
});
four_nations.model.map.enrichments.maybe_add_resource_to_tile = (function four_nations$model$map$enrichments$maybe_add_resource_to_tile(tile,p__39729){
var map__39730 = p__39729;
var map__39730__$1 = (((((!((map__39730 == null))))?(((((map__39730.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39730.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39730):map__39730);
var resource = map__39730__$1;
var spawn_predicate = cljs.core.get.call(null,map__39730__$1,new cljs.core.Keyword(null,"spawn-predicate","spawn-predicate",543545894));
if(cljs.core.truth_((((cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"resource","resource",251898836)], null)) == null))?spawn_predicate.call(null,tile):false))){
return cljs.core.assoc_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"resource","resource",251898836)], null),resource);
} else {
return tile;
}
});
/**
 * Given a map, its dimensions and available resources; adds resources to the map based on the
 * specified spawn predicates for those resources.
 */
four_nations.model.map.enrichments.add_resources = (function four_nations$model$map$enrichments$add_resources(m,dimensions,p__39732){
var map__39733 = p__39732;
var map__39733__$1 = (((((!((map__39733 == null))))?(((((map__39733.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39733.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39733):map__39733);
var resources = cljs.core.get.call(null,map__39733__$1,new cljs.core.Keyword(null,"resources","resources",1632806811));
return four_nations.model.map.utils.map_over_tiles.call(null,(function (tile){
return cljs.core.reduce.call(null,four_nations.model.map.enrichments.maybe_add_resource_to_tile,tile,resources);
}),m,dimensions);
});
/**
 * Given the noise map and the average value of cells across the map, builds basic game map with
 * simple terrain selection.
 */
four_nations.model.map.enrichments.add_basic_terrain = (function four_nations$model$map$enrichments$add_basic_terrain(noise,average_value){
return cljs.core.map.call(null,(function (p__39735){
var vec__39736 = p__39735;
var point = cljs.core.nth.call(null,vec__39736,(0),null);
var tile = cljs.core.nth.call(null,vec__39736,(1),null);
var terrain_type = four_nations.model.map.enrichments.determine_cell_terrain_type.call(null,new cljs.core.Keyword(null,"value","value",305978217).cljs$core$IFn$_invoke$arity$1(tile),average_value);
return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [point,cljs.core.assoc_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111)], null),terrain_type)], null);
}),noise);
});
/**
 * Adds biomes to the map through random subdivision.
 */
four_nations.model.map.enrichments.add_biomes = (function four_nations$model$map$enrichments$add_biomes(m,dimensions,p__39739){
var map__39740 = p__39739;
var map__39740__$1 = (((((!((map__39740 == null))))?(((((map__39740.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39740.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39740):map__39740);
var biomes = cljs.core.get.call(null,map__39740__$1,new cljs.core.Keyword(null,"biomes","biomes",1444240594));
var biome_count = cljs.core.get.call(null,map__39740__$1,new cljs.core.Keyword(null,"biome-count","biome-count",1287206126));
return four_nations.model.map.biomes.add_biomes.call(null,m,dimensions,biome_count,biomes);
});
four_nations.model.map.enrichments.default_biomes = cljs.core.map.call(null,four_nations.model.map.biomes.map__GT_Biome,new cljs.core.PersistentVector(null, 4, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"plains","plains",208589091),new cljs.core.Keyword(null,"display-name","display-name",694513143),"Plains"], null),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"forest","forest",278860306),new cljs.core.Keyword(null,"display-name","display-name",694513143),"Forest"], null),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"desert","desert",-559764082),new cljs.core.Keyword(null,"display-name","display-name",694513143),"Desert"], null),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"tundra","tundra",-1562952852),new cljs.core.Keyword(null,"display-name","display-name",694513143),"Tundra"], null)], null));
four_nations.model.map.enrichments.default_resources = cljs.core.map.call(null,four_nations.model.map.resources.resource_definition__GT_resource,new cljs.core.PersistentVector(null, 4, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.PersistentArrayMap(null, 5, [new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"gold-vein","gold-vein",44457129),new cljs.core.Keyword(null,"display-name","display-name",694513143),"Gold Vein",new cljs.core.Keyword(null,"symbol","symbol",-1038572696),"$",new cljs.core.Keyword(null,"spawning","spawning",-243970190),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"chance","chance",-1877227883),0.01,new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"terrain-types","terrain-types",507721131),new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"land","land",-1402569867),null,new cljs.core.Keyword(null,"mountain","mountain",-1770530243),null], null), null)], null)], null),new cljs.core.Keyword(null,"image","image",-58725096),"images/resources/gold.png"], null),new cljs.core.PersistentArrayMap(null, 5, [new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"oak-tree","oak-tree",-2086402672),new cljs.core.Keyword(null,"display-name","display-name",694513143),"Oak Tree",new cljs.core.Keyword(null,"symbol","symbol",-1038572696),"T",new cljs.core.Keyword(null,"spawning","spawning",-243970190),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"chance","chance",-1877227883),0.3,new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"terrain-types","terrain-types",507721131),new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"land","land",-1402569867),null], null), null),new cljs.core.Keyword(null,"biomes","biomes",1444240594),new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"plains","plains",208589091),null], null), null)], null)], null),new cljs.core.Keyword(null,"image","image",-58725096),new cljs.core.Keyword(null,"oak-tree","oak-tree",-2086402672)], null),new cljs.core.PersistentArrayMap(null, 5, [new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"fir-tree","fir-tree",-1367923778),new cljs.core.Keyword(null,"display-name","display-name",694513143),"Fir Tree",new cljs.core.Keyword(null,"symbol","symbol",-1038572696),"F",new cljs.core.Keyword(null,"spawning","spawning",-243970190),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"chance","chance",-1877227883),0.9,new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"terrain-types","terrain-types",507721131),new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"land","land",-1402569867),null], null), null),new cljs.core.Keyword(null,"biomes","biomes",1444240594),new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"forest","forest",278860306),null], null), null)], null)], null),new cljs.core.Keyword(null,"image","image",-58725096),new cljs.core.Keyword(null,"fir-tree","fir-tree",-1367923778)], null),new cljs.core.PersistentArrayMap(null, 5, [new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"oil-deposit","oil-deposit",2121051936),new cljs.core.Keyword(null,"display-name","display-name",694513143),"Oil Deposit",new cljs.core.Keyword(null,"symbol","symbol",-1038572696),"O",new cljs.core.Keyword(null,"spawning","spawning",-243970190),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"chance","chance",-1877227883),0.005,new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"terrain-types","terrain-types",507721131),new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"water","water",-824098213),null], null), null)], null)], null),new cljs.core.Keyword(null,"image","image",-58725096),"images/resources/oil.png"], null)], null));
four_nations.model.map.enrichments.default_enrichment_options = new cljs.core.PersistentArrayMap(null, 5, [new cljs.core.Keyword(null,"water-spread-chance","water-spread-chance",-136332362),0.1,new cljs.core.Keyword(null,"water-border","water-border",23035014),(2),new cljs.core.Keyword(null,"biome-count","biome-count",1287206126),((5) * cljs.core.count.call(null,four_nations.model.map.enrichments.default_biomes)),new cljs.core.Keyword(null,"biomes","biomes",1444240594),four_nations.model.map.enrichments.default_biomes,new cljs.core.Keyword(null,"resources","resources",1632806811),four_nations.model.map.enrichments.default_resources], null);
/**
 * Given a generated and smoothed noisemap, generates a game map with additional attributes.
 */
four_nations.model.map.enrichments.enrich_noise_map = (function four_nations$model$map$enrichments$enrich_noise_map(var_args){
var G__39743 = arguments.length;
switch (G__39743) {
case 2:
return four_nations.model.map.enrichments.enrich_noise_map.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return four_nations.model.map.enrichments.enrich_noise_map.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.enrichments.enrich_noise_map.cljs$core$IFn$_invoke$arity$2 = (function (noise,dimensions){
return four_nations.model.map.enrichments.enrich_noise_map.call(null,noise,dimensions,four_nations.model.map.enrichments.default_enrichment_options);
}));

(four_nations.model.map.enrichments.enrich_noise_map.cljs$core$IFn$_invoke$arity$3 = (function (noise,dimensions,options){
var average_value = four_nations.model.map.enrichments.average_map_value.call(null,noise);
return four_nations.model.map.enrichments.add_resources.call(null,four_nations.model.map.enrichments.add_coastline.call(null,four_nations.model.map.enrichments.drench_surrounded_land.call(null,four_nations.model.map.enrichments.spread_water.call(null,four_nations.model.map.enrichments.add_water_border.call(null,four_nations.model.map.enrichments.add_biomes.call(null,four_nations.model.map.enrichments.add_basic_terrain.call(null,noise,average_value),dimensions,options),dimensions,options),dimensions,options),dimensions),dimensions),dimensions,options);
}));

(four_nations.model.map.enrichments.enrich_noise_map.cljs$lang$maxFixedArity = 3);


//# sourceMappingURL=enrichments.js.map?rel=1654487547754
