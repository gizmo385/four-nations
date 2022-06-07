// Compiled by ClojureScript 1.10.773 {}
goog.provide('four_nations.core');
goog.require('cljs.core');
goog.require('four_nations.model.map');
goog.require('four_nations.model.map.utils');
goog.require('four_nations.model.map.enrichments');
goog.require('four_nations.general.types');
goog.require('reagent.core');
goog.require('reagent_modals.modals');
goog.require('goog.string');
goog.require('goog.string.format');
goog.require('reagent.dom');
cljs.core.enable_console_print_BANG_.call(null);
if((typeof four_nations !== 'undefined') && (typeof four_nations.core !== 'undefined') && (typeof four_nations.core.game_generation_settings !== 'undefined')){
} else {
four_nations.core.game_generation_settings = reagent.core.atom.call(null,new cljs.core.PersistentArrayMap(null, 4, [new cljs.core.Keyword(null,"enrichments","enrichments",-2086384792),four_nations.model.map.enrichments.default_enrichment_options,new cljs.core.Keyword(null,"map-height","map-height",1060022070),(50),new cljs.core.Keyword(null,"map-width","map-width",-1419340239),(50),new cljs.core.Keyword(null,"smoothing-passes","smoothing-passes",1929301954),(15)], null));
}
if((typeof four_nations !== 'undefined') && (typeof four_nations.core !== 'undefined') && (typeof four_nations.core.game_state !== 'undefined')){
} else {
four_nations.core.game_state = reagent.core.atom.call(null,new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"game-map","game-map",-1905642262),null,new cljs.core.Keyword(null,"dimension","dimension",543254198),null,new cljs.core.Keyword(null,"tile-size","tile-size",441420757),(32)], null));
}
four_nations.core.tile_detail = (function four_nations$core$tile_detail(tile){
return new cljs.core.PersistentVector(null, 6, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"div","div",1057191632),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"style","style",-496642736),new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"display","display",242065432),new cljs.core.Keyword(null,"inline-block","inline-block",1967810016),new cljs.core.Keyword(null,"border","border",1444987323),"1px solid black",new cljs.core.Keyword(null,"margin","margin",-995903681),new cljs.core.Keyword(null,"auto","auto",-566279492)], null)], null),new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"p","p",151049309),goog.string.format("Tile @ (%s, %s)",new cljs.core.Keyword(null,"x","x",2099068185).cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"point","point",1813198264).cljs$core$IFn$_invoke$arity$1(tile)),new cljs.core.Keyword(null,"y","y",-1757859776).cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"point","point",1813198264).cljs$core$IFn$_invoke$arity$1(tile)))], null),new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"p","p",151049309),goog.string.format("Terrain Type: %s",cljs.core.name.call(null,new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111).cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"attributes","attributes",-74013604).cljs$core$IFn$_invoke$arity$1(tile))))], null),new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"p","p",151049309),goog.string.format("Biome: %s",new cljs.core.Keyword(null,"display-name","display-name",694513143).cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"biome","biome",1856013988).cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"attributes","attributes",-74013604).cljs$core$IFn$_invoke$arity$1(tile))))], null),new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"p","p",151049309),goog.string.format("Resource: %s",cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"resource","resource",251898836),new cljs.core.Keyword(null,"display-name","display-name",694513143)], null),"None"))], null)], null);
});
four_nations.core.biome_terrain_type_image_config = new cljs.core.PersistentArrayMap(null, 5, [new cljs.core.Keyword(null,"tundra","tundra",-1562952852),new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"land","land",-1402569867),"images/tiles/ice.png",new cljs.core.Keyword(null,"water","water",-824098213),"images/tiles/water.png",new cljs.core.Keyword(null,"coast","coast",1578056704),"images/tiles/coast.png"], null),new cljs.core.Keyword(null,"forest","forest",278860306),new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"land","land",-1402569867),"images/tiles/dark-grass.png",new cljs.core.Keyword(null,"water","water",-824098213),"images/tiles/water.png",new cljs.core.Keyword(null,"coast","coast",1578056704),"images/tiles/coast.png"], null),new cljs.core.Keyword(null,"desert","desert",-559764082),new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"land","land",-1402569867),"images/tiles/sand.png",new cljs.core.Keyword(null,"water","water",-824098213),"images/tiles/water.png",new cljs.core.Keyword(null,"coast","coast",1578056704),"images/tiles/coast.png"], null),new cljs.core.Keyword(null,"plains","plains",208589091),new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"land","land",-1402569867),"images/tiles/grass.png",new cljs.core.Keyword(null,"water","water",-824098213),"images/tiles/water.png",new cljs.core.Keyword(null,"coast","coast",1578056704),"images/tiles/coast.png"], null),null,new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"land","land",-1402569867),"images/tiles/grass.png",new cljs.core.Keyword(null,"water","water",-824098213),"images/tiles/water.png",new cljs.core.Keyword(null,"coast","coast",1578056704),"images/tiles/coast.png"], null)], null);
four_nations.core.resource_image_config = new cljs.core.PersistentArrayMap(null, 4, [new cljs.core.Keyword(null,"oak-tree","oak-tree",-2086402672),"images/resources/oak-tree.png",new cljs.core.Keyword(null,"fir-tree","fir-tree",-1367923778),"images/resources/fir-tree.png",new cljs.core.Keyword(null,"gold-vein","gold-vein",44457129),"images/resources/gold.png",new cljs.core.Keyword(null,"oil-deposit","oil-deposit",2121051936),"images/resources/oil.png"], null);
four_nations.core.tile_resource_image = (function four_nations$core$tile_resource_image(tile,tile_size){
var temp__5718__auto__ = four_nations.core.resource_image_config.call(null,new cljs.core.Keyword(null,"name","name",1843675177).cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"resource","resource",251898836).cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"attributes","attributes",-74013604).cljs$core$IFn$_invoke$arity$1(tile))));
if(cljs.core.truth_(temp__5718__auto__)){
var resource_image = temp__5718__auto__;
return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"img","img",1442687358),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"style","style",-496642736),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"position","position",-2011731912),new cljs.core.Keyword(null,"absolute","absolute",1655386478)], null),new cljs.core.Keyword(null,"src","src",-1651076051),resource_image], null)], null);
} else {
return null;
}
});
four_nations.core.tile_terrain_image = (function four_nations$core$tile_terrain_image(tile,tile_size){
var terrain_type = cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111)], null));
var biome = cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"biome","biome",1856013988),new cljs.core.Keyword(null,"name","name",1843675177)], null));
var temp__5718__auto__ = cljs.core.get_in.call(null,four_nations.core.biome_terrain_type_image_config,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [biome,terrain_type], null));
if(cljs.core.truth_(temp__5718__auto__)){
var image = temp__5718__auto__;
return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"img","img",1442687358),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"style","style",-496642736),new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"position","position",-2011731912),new cljs.core.Keyword(null,"absolute","absolute",1655386478),new cljs.core.Keyword(null,"height","height",1025178622),tile_size,new cljs.core.Keyword(null,"width","width",-384071477),tile_size], null),new cljs.core.Keyword(null,"src","src",-1651076051),image], null)], null);
} else {
return null;
}
});
/**
 * Render a single map tile
 */
four_nations.core.map_tile = (function four_nations$core$map_tile(t){
var coordinates = new cljs.core.Keyword(null,"point","point",1813198264).cljs$core$IFn$_invoke$arity$1(t);
var tile_size = new cljs.core.Keyword(null,"tile-size","tile-size",441420757).cljs$core$IFn$_invoke$arity$1(cljs.core.deref.call(null,four_nations.core.game_state));
return new cljs.core.PersistentVector(null, 4, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"a","a",-2123407586),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"style","style",-496642736),new cljs.core.PersistentArrayMap(null, 6, [new cljs.core.Keyword(null,"height","height",1025178622),tile_size,new cljs.core.Keyword(null,"width","width",-384071477),tile_size,new cljs.core.Keyword(null,"border","border",1444987323),"1px solid black",new cljs.core.Keyword(null,"display","display",242065432),new cljs.core.Keyword(null,"inline-block","inline-block",1967810016),new cljs.core.Keyword(null,"font-family","font-family",-667419874),"monospace",new cljs.core.Keyword(null,"background-color","background-color",570434026),four_nations.model.map.utils.terrain_type__GT_color.call(null,new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111).cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"attributes","attributes",-74013604).cljs$core$IFn$_invoke$arity$1(t)))], null),new cljs.core.Keyword(null,"on-click","on-click",1632826543),(function (_){
return reagent_modals.modals.modal_BANG_.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.tile_detail,t], null));
})], null),new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.tile_terrain_image,t,tile_size], null),new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.tile_resource_image,t,tile_size], null)], null);
});
/**
 * Render the entire map based on the current state
 */
four_nations.core.game_map_display = (function four_nations$core$game_map_display(game_map,dimension){
return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"div","div",1057191632),(function (){var iter__4529__auto__ = (function four_nations$core$game_map_display_$_iter__29622(s__29623){
return (new cljs.core.LazySeq(null,(function (){
var s__29623__$1 = s__29623;
while(true){
var temp__5720__auto__ = cljs.core.seq.call(null,s__29623__$1);
if(temp__5720__auto__){
var s__29623__$2 = temp__5720__auto__;
if(cljs.core.chunked_seq_QMARK_.call(null,s__29623__$2)){
var c__4527__auto__ = cljs.core.chunk_first.call(null,s__29623__$2);
var size__4528__auto__ = cljs.core.count.call(null,c__4527__auto__);
var b__29625 = cljs.core.chunk_buffer.call(null,size__4528__auto__);
if((function (){var i__29624 = (0);
while(true){
if((i__29624 < size__4528__auto__)){
var y = cljs.core._nth.call(null,c__4527__auto__,i__29624);
cljs.core.chunk_append.call(null,b__29625,cljs.core.with_meta(new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"div","div",1057191632),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"class","class",-2030961996),"map-row"], null),(function (){var iter__4529__auto__ = ((function (i__29624,y,c__4527__auto__,size__4528__auto__,b__29625,s__29623__$2,temp__5720__auto__){
return (function four_nations$core$game_map_display_$_iter__29622_$_iter__29626(s__29627){
return (new cljs.core.LazySeq(null,((function (i__29624,y,c__4527__auto__,size__4528__auto__,b__29625,s__29623__$2,temp__5720__auto__){
return (function (){
var s__29627__$1 = s__29627;
while(true){
var temp__5720__auto____$1 = cljs.core.seq.call(null,s__29627__$1);
if(temp__5720__auto____$1){
var s__29627__$2 = temp__5720__auto____$1;
if(cljs.core.chunked_seq_QMARK_.call(null,s__29627__$2)){
var c__4527__auto____$1 = cljs.core.chunk_first.call(null,s__29627__$2);
var size__4528__auto____$1 = cljs.core.count.call(null,c__4527__auto____$1);
var b__29629 = cljs.core.chunk_buffer.call(null,size__4528__auto____$1);
if((function (){var i__29628 = (0);
while(true){
if((i__29628 < size__4528__auto____$1)){
var x = cljs.core._nth.call(null,c__4527__auto____$1,i__29628);
cljs.core.chunk_append.call(null,b__29629,cljs.core.with_meta(new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.map_tile,four_nations.model.map.utils.get_cell.call(null,game_map,four_nations.general.types.__GT_Point.call(null,x,y))], null),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"key","key",-1516042587),goog.string.format("GameMapTile_y=%s_x=%s",y,x)], null)));

var G__29634 = (i__29628 + (1));
i__29628 = G__29634;
continue;
} else {
return true;
}
break;
}
})()){
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__29629),four_nations$core$game_map_display_$_iter__29622_$_iter__29626.call(null,cljs.core.chunk_rest.call(null,s__29627__$2)));
} else {
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__29629),null);
}
} else {
var x = cljs.core.first.call(null,s__29627__$2);
return cljs.core.cons.call(null,cljs.core.with_meta(new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.map_tile,four_nations.model.map.utils.get_cell.call(null,game_map,four_nations.general.types.__GT_Point.call(null,x,y))], null),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"key","key",-1516042587),goog.string.format("GameMapTile_y=%s_x=%s",y,x)], null)),four_nations$core$game_map_display_$_iter__29622_$_iter__29626.call(null,cljs.core.rest.call(null,s__29627__$2)));
}
} else {
return null;
}
break;
}
});})(i__29624,y,c__4527__auto__,size__4528__auto__,b__29625,s__29623__$2,temp__5720__auto__))
,null,null));
});})(i__29624,y,c__4527__auto__,size__4528__auto__,b__29625,s__29623__$2,temp__5720__auto__))
;
return iter__4529__auto__.call(null,cljs.core.range.call(null,new cljs.core.Keyword(null,"width","width",-384071477).cljs$core$IFn$_invoke$arity$1(dimension)));
})()], null),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"key","key",-1516042587),goog.string.format("GameMapRow_y=%s",y)], null)));

var G__29635 = (i__29624 + (1));
i__29624 = G__29635;
continue;
} else {
return true;
}
break;
}
})()){
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__29625),four_nations$core$game_map_display_$_iter__29622.call(null,cljs.core.chunk_rest.call(null,s__29623__$2)));
} else {
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__29625),null);
}
} else {
var y = cljs.core.first.call(null,s__29623__$2);
return cljs.core.cons.call(null,cljs.core.with_meta(new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"div","div",1057191632),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"class","class",-2030961996),"map-row"], null),(function (){var iter__4529__auto__ = ((function (y,s__29623__$2,temp__5720__auto__){
return (function four_nations$core$game_map_display_$_iter__29622_$_iter__29630(s__29631){
return (new cljs.core.LazySeq(null,(function (){
var s__29631__$1 = s__29631;
while(true){
var temp__5720__auto____$1 = cljs.core.seq.call(null,s__29631__$1);
if(temp__5720__auto____$1){
var s__29631__$2 = temp__5720__auto____$1;
if(cljs.core.chunked_seq_QMARK_.call(null,s__29631__$2)){
var c__4527__auto__ = cljs.core.chunk_first.call(null,s__29631__$2);
var size__4528__auto__ = cljs.core.count.call(null,c__4527__auto__);
var b__29633 = cljs.core.chunk_buffer.call(null,size__4528__auto__);
if((function (){var i__29632 = (0);
while(true){
if((i__29632 < size__4528__auto__)){
var x = cljs.core._nth.call(null,c__4527__auto__,i__29632);
cljs.core.chunk_append.call(null,b__29633,cljs.core.with_meta(new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.map_tile,four_nations.model.map.utils.get_cell.call(null,game_map,four_nations.general.types.__GT_Point.call(null,x,y))], null),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"key","key",-1516042587),goog.string.format("GameMapTile_y=%s_x=%s",y,x)], null)));

var G__29636 = (i__29632 + (1));
i__29632 = G__29636;
continue;
} else {
return true;
}
break;
}
})()){
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__29633),four_nations$core$game_map_display_$_iter__29622_$_iter__29630.call(null,cljs.core.chunk_rest.call(null,s__29631__$2)));
} else {
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__29633),null);
}
} else {
var x = cljs.core.first.call(null,s__29631__$2);
return cljs.core.cons.call(null,cljs.core.with_meta(new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.map_tile,four_nations.model.map.utils.get_cell.call(null,game_map,four_nations.general.types.__GT_Point.call(null,x,y))], null),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"key","key",-1516042587),goog.string.format("GameMapTile_y=%s_x=%s",y,x)], null)),four_nations$core$game_map_display_$_iter__29622_$_iter__29630.call(null,cljs.core.rest.call(null,s__29631__$2)));
}
} else {
return null;
}
break;
}
}),null,null));
});})(y,s__29623__$2,temp__5720__auto__))
;
return iter__4529__auto__.call(null,cljs.core.range.call(null,new cljs.core.Keyword(null,"width","width",-384071477).cljs$core$IFn$_invoke$arity$1(dimension)));
})()], null),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"key","key",-1516042587),goog.string.format("GameMapRow_y=%s",y)], null)),four_nations$core$game_map_display_$_iter__29622.call(null,cljs.core.rest.call(null,s__29623__$2)));
}
} else {
return null;
}
break;
}
}),null,null));
});
return iter__4529__auto__.call(null,cljs.core.range.call(null,new cljs.core.Keyword(null,"height","height",1025178622).cljs$core$IFn$_invoke$arity$1(dimension)));
})()], null);
});
/**
 * Handler for re-generating the map based on the current settings
 */
four_nations.core.re_generate_map = (function four_nations$core$re_generate_map(_){
var map__29637 = cljs.core.deref.call(null,four_nations.core.game_generation_settings);
var map__29637__$1 = (((((!((map__29637 == null))))?(((((map__29637.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__29637.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__29637):map__29637);
var settings = map__29637__$1;
var enrichments = cljs.core.get.call(null,map__29637__$1,new cljs.core.Keyword(null,"enrichments","enrichments",-2086384792));
var map_height = cljs.core.get.call(null,map__29637__$1,new cljs.core.Keyword(null,"map-height","map-height",1060022070));
var map_width = cljs.core.get.call(null,map__29637__$1,new cljs.core.Keyword(null,"map-width","map-width",-1419340239));
var smoothing_passes = cljs.core.get.call(null,map__29637__$1,new cljs.core.Keyword(null,"smoothing-passes","smoothing-passes",1929301954));
var dimension = four_nations.general.types.__GT_Dimension.call(null,map_height,map_width);
return cljs.core.swap_BANG_.call(null,four_nations.core.game_state,cljs.core.assoc,new cljs.core.Keyword(null,"dimension","dimension",543254198),dimension,new cljs.core.Keyword(null,"game-map","game-map",-1905642262),(function (){var start__4626__auto__ = cljs.core.system_time.call(null);
var ret__4627__auto__ = four_nations.model.map.build_map.call(null,dimension,smoothing_passes,enrichments);
cljs.core.prn.call(null,["Elapsed time: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1((cljs.core.system_time.call(null) - start__4626__auto__).toFixed((6)))," msecs"].join(''));

return ret__4627__auto__;
})());
});
/**
 * Updates the map generation settings based on an event from a range input
 */
four_nations.core.update_generation_setting = (function four_nations$core$update_generation_setting(attribute_to_update,event,type_fn){
var new_value = type_fn.call(null,event.target.value);
return cljs.core.swap_BANG_.call(null,four_nations.core.game_generation_settings,cljs.core.assoc_in,attribute_to_update,new_value);
});
/**
 * A type-aware slider element that is labeled with the current value
 */
four_nations.core.labeled_settings_slider = (function four_nations$core$labeled_settings_slider(label,attr_path,min_value,max_value,step,clj_type_fn,js_type_fn){
var value = cljs.core.get_in.call(null,cljs.core.deref.call(null,four_nations.core.game_generation_settings),attr_path);
return new cljs.core.PersistentVector(null, 6, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"div","div",1057191632),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"style","style",-496642736),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"width","width",-384071477),"50%"], null)], null),label,": ",new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"input","input",556931961),new cljs.core.PersistentArrayMap(null, 6, [new cljs.core.Keyword(null,"type","type",1174270348),"range",new cljs.core.Keyword(null,"value","value",305978217),value,new cljs.core.Keyword(null,"min","min",444991522),min_value,new cljs.core.Keyword(null,"max","max",61366548),max_value,new cljs.core.Keyword(null,"step","step",1288888124),step,new cljs.core.Keyword(null,"on-change","on-change",-732046149),(function (e){
return four_nations.core.update_generation_setting.call(null,attr_path,e,js_type_fn);
})], null)], null),clj_type_fn.call(null,value)], null);
});
/**
 * UI elements for altering the generation settings of the map
 */
four_nations.core.game_setting_controls = (function four_nations$core$game_setting_controls(){
return new cljs.core.PersistentVector(null, 10, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"div","div",1057191632),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"style","style",-496642736),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"margin-bottom","margin-bottom",388334941),"20px"], null)], null),new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"h3","h3",2067611163),"Game Settings"], null),new cljs.core.PersistentVector(null, 8, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.labeled_settings_slider,"Height",new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"map-height","map-height",1060022070)], null),(10),(500),(1),cljs.core.int$,parseInt], null),new cljs.core.PersistentVector(null, 8, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.labeled_settings_slider,"Width",new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"map-width","map-width",-1419340239)], null),(10),(500),(1),cljs.core.int$,parseInt], null),new cljs.core.PersistentVector(null, 8, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.labeled_settings_slider,"Smoothing Passes",new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"smoothing-passes","smoothing-passes",1929301954)], null),(1),(30),(1),cljs.core.int$,parseInt], null),new cljs.core.PersistentVector(null, 8, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.labeled_settings_slider,"Water Spread Chance",new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"enrichments","enrichments",-2086384792),new cljs.core.Keyword(null,"water-spread-chance","water-spread-chance",-136332362)], null),0.01,(1),0.01,cljs.core.float$,parseFloat], null),new cljs.core.PersistentVector(null, 8, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.labeled_settings_slider,"Water Border",new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"enrichments","enrichments",-2086384792),new cljs.core.Keyword(null,"water-border","water-border",23035014)], null),(1),(10),(1),cljs.core.int$,parseInt], null),new cljs.core.PersistentVector(null, 8, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.labeled_settings_slider,"Biome Count",new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"enrichments","enrichments",-2086384792),new cljs.core.Keyword(null,"biome-count","biome-count",1287206126)], null),(1),(50),(1),cljs.core.int$,parseInt], null),new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"button.green","button.green",-1328299291),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"on-click","on-click",1632826543),four_nations.core.re_generate_map], null),"Re-generate Map"], null)], null);
});
four_nations.core.game_map_or_loading = (function four_nations$core$game_map_or_loading(){
var map__29639 = cljs.core.deref.call(null,four_nations.core.game_state);
var map__29639__$1 = (((((!((map__29639 == null))))?(((((map__29639.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__29639.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__29639):map__29639);
var game_map = cljs.core.get.call(null,map__29639__$1,new cljs.core.Keyword(null,"game-map","game-map",-1905642262));
var dimension = cljs.core.get.call(null,map__29639__$1,new cljs.core.Keyword(null,"dimension","dimension",543254198));
return new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.game_map_display,game_map,dimension], null);
});
four_nations.core.game_display = (function four_nations$core$game_display(){
return new cljs.core.PersistentVector(null, 4, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"div","div",1057191632),new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [reagent_modals.modals.modal_window], null),new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.game_setting_controls], null),new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.game_map_or_loading], null)], null);
});
reagent.dom.render.call(null,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.core.game_display], null),document.getElementById("app"));
four_nations.core.on_js_reload = (function four_nations$core$on_js_reload(){
return null;
});

//# sourceMappingURL=core.js.map?rel=1654579642642
