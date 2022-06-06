// Compiled by ClojureScript 1.10.773 {}
goog.provide('four_nations.model.map.utils');
goog.require('cljs.core');
goog.require('clojure.string');
goog.require('four_nations.general.utils');
goog.require('four_nations.general.types');
/**
 * Given a 2 dimensional game map and an (x, y) coordinate pair, retrieves the value at the
 * specified cell. If an invalid x/y pair are passed in, this function will return nil.
 */
four_nations.model.map.utils.get_cell = (function four_nations$model$map$utils$get_cell(var_args){
var G__39558 = arguments.length;
switch (G__39558) {
case 2:
return four_nations.model.map.utils.get_cell.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return four_nations.model.map.utils.get_cell.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.utils.get_cell.cljs$core$IFn$_invoke$arity$2 = (function (m,p){
return cljs.core.get.call(null,m,p);
}));

(four_nations.model.map.utils.get_cell.cljs$core$IFn$_invoke$arity$3 = (function (m,x,y){
return cljs.core.get.call(null,m,four_nations.general.types.__GT_Point.call(null,x,y));
}));

(four_nations.model.map.utils.get_cell.cljs$lang$maxFixedArity = 3);

/**
 * A coordinate is valid if both components are non-negative integers and they are both less than
 * their respective bounds (y < height, x < width). Assumes zero-indexed coordinates.
 */
four_nations.model.map.utils.valid_coordinate_QMARK_ = (function four_nations$model$map$utils$valid_coordinate_QMARK_(p__39560,p__39561){
var map__39562 = p__39560;
var map__39562__$1 = (((((!((map__39562 == null))))?(((((map__39562.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39562.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39562):map__39562);
var dimension = map__39562__$1;
var height = cljs.core.get.call(null,map__39562__$1,new cljs.core.Keyword(null,"height","height",1025178622));
var width = cljs.core.get.call(null,map__39562__$1,new cljs.core.Keyword(null,"width","width",-384071477));
var map__39563 = p__39561;
var map__39563__$1 = (((((!((map__39563 == null))))?(((((map__39563.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39563.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39563):map__39563);
var point = map__39563__$1;
var x = cljs.core.get.call(null,map__39563__$1,new cljs.core.Keyword(null,"x","x",2099068185));
var y = cljs.core.get.call(null,map__39563__$1,new cljs.core.Keyword(null,"y","y",-1757859776));
return ((cljs.core.nat_int_QMARK_.call(null,x)) && (cljs.core.nat_int_QMARK_.call(null,y)) && ((y < height)) && ((x < width)));
});
/**
 * Given some dimensions, returns a random point within those dimensions.
 */
four_nations.model.map.utils.random_point_in_dimensions = (function four_nations$model$map$utils$random_point_in_dimensions(dimensions){
return four_nations.general.types.__GT_Point.call(null,cljs.core.rand_int.call(null,new cljs.core.Keyword(null,"width","width",-384071477).cljs$core$IFn$_invoke$arity$1(dimensions)),cljs.core.rand_int.call(null,new cljs.core.Keyword(null,"height","height",1025178622).cljs$core$IFn$_invoke$arity$1(dimensions)));
});
/**
 * Given a height, a width, and a (x, y) coordinate pair, returns all valid coordinates that are
 * neighbors. Valid coordinates are checked against the valid-coordinate? predicate
 */
four_nations.model.map.utils.point__GT_neighbor_points_STAR_ = (function four_nations$model$map$utils$point__GT_neighbor_points_STAR_(var_args){
var G__39567 = arguments.length;
switch (G__39567) {
case 2:
return four_nations.model.map.utils.point__GT_neighbor_points_STAR_.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return four_nations.model.map.utils.point__GT_neighbor_points_STAR_.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.utils.point__GT_neighbor_points_STAR_.cljs$core$IFn$_invoke$arity$2 = (function (p__39568,p__39569){
var map__39570 = p__39568;
var map__39570__$1 = (((((!((map__39570 == null))))?(((((map__39570.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39570.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39570):map__39570);
var dimension = map__39570__$1;
var height = cljs.core.get.call(null,map__39570__$1,new cljs.core.Keyword(null,"height","height",1025178622));
var width = cljs.core.get.call(null,map__39570__$1,new cljs.core.Keyword(null,"width","width",-384071477));
var map__39571 = p__39569;
var map__39571__$1 = (((((!((map__39571 == null))))?(((((map__39571.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39571.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39571):map__39571);
var point = map__39571__$1;
var x = cljs.core.get.call(null,map__39571__$1,new cljs.core.Keyword(null,"x","x",2099068185));
var y = cljs.core.get.call(null,map__39571__$1,new cljs.core.Keyword(null,"y","y",-1757859776));
return four_nations.model.map.utils.point__GT_neighbor_points_STAR_.call(null,dimension,point,true);
}));

(four_nations.model.map.utils.point__GT_neighbor_points_STAR_.cljs$core$IFn$_invoke$arity$3 = (function (p__39574,p__39575,include_diagonal_QMARK_){
var map__39576 = p__39574;
var map__39576__$1 = (((((!((map__39576 == null))))?(((((map__39576.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39576.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39576):map__39576);
var dimension = map__39576__$1;
var height = cljs.core.get.call(null,map__39576__$1,new cljs.core.Keyword(null,"height","height",1025178622));
var width = cljs.core.get.call(null,map__39576__$1,new cljs.core.Keyword(null,"width","width",-384071477));
var map__39577 = p__39575;
var map__39577__$1 = (((((!((map__39577 == null))))?(((((map__39577.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__39577.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__39577):map__39577);
var point = map__39577__$1;
var x = cljs.core.get.call(null,map__39577__$1,new cljs.core.Keyword(null,"x","x",2099068185));
var y = cljs.core.get.call(null,map__39577__$1,new cljs.core.Keyword(null,"y","y",-1757859776));
var cardinal_neighbors = new cljs.core.PersistentVector(null, 4, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.general.types.__GT_Point.call(null,x,(y + (1))),four_nations.general.types.__GT_Point.call(null,x,(y - (1))),four_nations.general.types.__GT_Point.call(null,(x + (1)),y),four_nations.general.types.__GT_Point.call(null,(x - (1)),y)], null);
var diagonal_neighbors = new cljs.core.PersistentVector(null, 4, 5, cljs.core.PersistentVector.EMPTY_NODE, [four_nations.general.types.__GT_Point.call(null,(x + (1)),(y + (1))),four_nations.general.types.__GT_Point.call(null,(x + (1)),(y - (1))),four_nations.general.types.__GT_Point.call(null,(x - (1)),(y + (1))),four_nations.general.types.__GT_Point.call(null,(x - (1)),(y - (1)))], null);
return cljs.core.set.call(null,cljs.core.filter.call(null,cljs.core.partial.call(null,four_nations.model.map.utils.valid_coordinate_QMARK_,dimension),cljs.core.concat.call(null,(cljs.core.truth_(include_diagonal_QMARK_)?diagonal_neighbors:null),cardinal_neighbors)));
}));

(four_nations.model.map.utils.point__GT_neighbor_points_STAR_.cljs$lang$maxFixedArity = 3);

/**
 * Given a height, a width, and a (x, y) coordinate pair, returns all valid coordinates that are
 * neighbors. Valid coordinates are checked against the valid-coordinate? predicate
 */
four_nations.model.map.utils.point__GT_neighbor_points = cljs.core.memoize.call(null,four_nations.model.map.utils.point__GT_neighbor_points_STAR_);
/**
 * Given a height, a width, a map m, and a (x, y) coordinate pair, returns the values of all valid
 * neighbors, as defined by point->neighbor-coordinates.
 */
four_nations.model.map.utils.point__GT_neighbors = (function four_nations$model$map$utils$point__GT_neighbors(var_args){
var G__39582 = arguments.length;
switch (G__39582) {
case 3:
return four_nations.model.map.utils.point__GT_neighbors.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
case 4:
return four_nations.model.map.utils.point__GT_neighbors.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.utils.point__GT_neighbors.cljs$core$IFn$_invoke$arity$3 = (function (dimension,m,point){
return four_nations.model.map.utils.point__GT_neighbors.call(null,dimension,m,point,true);
}));

(four_nations.model.map.utils.point__GT_neighbors.cljs$core$IFn$_invoke$arity$4 = (function (dimension,m,point,include_diagonal_QMARK_){
return cljs.core.map.call(null,cljs.core.partial.call(null,four_nations.model.map.utils.get_cell,m),four_nations.model.map.utils.point__GT_neighbor_points.call(null,dimension,point,include_diagonal_QMARK_));
}));

(four_nations.model.map.utils.point__GT_neighbors.cljs$lang$maxFixedArity = 4);

four_nations.model.map.utils.map_over_tiles = (function four_nations$model$map$utils$map_over_tiles(f,m,dimension){
return cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,cljs.core.map.call(null,(function (p__39584){
var vec__39585 = p__39584;
var point = cljs.core.nth.call(null,vec__39585,(0),null);
var tile = cljs.core.nth.call(null,vec__39585,(1),null);
return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [point,f.call(null,tile)], null);
}),m));
});
four_nations.model.map.utils.randomly_map_over_tiles = (function four_nations$model$map$utils$randomly_map_over_tiles(f,m,dimension){
var shuffled_map = cljs.core.shuffle.call(null,cljs.core.vec.call(null,m));
return four_nations.model.map.utils.map_over_tiles.call(null,f,shuffled_map,dimension);
});
/**
 * Applies f to every neighbor of the tile.
 */
four_nations.model.map.utils.map_over_neighbors = (function four_nations$model$map$utils$map_over_neighbors(var_args){
var G__39589 = arguments.length;
switch (G__39589) {
case 4:
return four_nations.model.map.utils.map_over_neighbors.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
case 5:
return four_nations.model.map.utils.map_over_neighbors.cljs$core$IFn$_invoke$arity$5((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]),(arguments[(4)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.utils.map_over_neighbors.cljs$core$IFn$_invoke$arity$4 = (function (f,tile,m,dimensions){
return four_nations.model.map.utils.map_over_neighbors.call(null,f,tile,m,dimensions,true);
}));

(four_nations.model.map.utils.map_over_neighbors.cljs$core$IFn$_invoke$arity$5 = (function (f,tile,m,dimensions,include_diagonals_QMARK_){
var neighbors = four_nations.model.map.utils.point__GT_neighbors.call(null,dimensions,m,new cljs.core.Keyword(null,"point","point",1813198264).cljs$core$IFn$_invoke$arity$1(tile),include_diagonals_QMARK_);
return cljs.core.map.call(null,f,neighbors);
}));

(four_nations.model.map.utils.map_over_neighbors.cljs$lang$maxFixedArity = 5);

/**
 * Applies f to every neighbor of the tile in a random order.
 */
four_nations.model.map.utils.randomly_map_over_neighbors = (function four_nations$model$map$utils$randomly_map_over_neighbors(var_args){
var G__39592 = arguments.length;
switch (G__39592) {
case 4:
return four_nations.model.map.utils.randomly_map_over_neighbors.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
case 5:
return four_nations.model.map.utils.randomly_map_over_neighbors.cljs$core$IFn$_invoke$arity$5((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]),(arguments[(4)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.utils.randomly_map_over_neighbors.cljs$core$IFn$_invoke$arity$4 = (function (f,tile,m,dimensions){
return four_nations.model.map.utils.map_over_neighbors.call(null,f,tile,m,dimensions,true);
}));

(four_nations.model.map.utils.randomly_map_over_neighbors.cljs$core$IFn$_invoke$arity$5 = (function (f,tile,m,dimensions,include_diagonals_QMARK_){
var neighbors = four_nations.model.map.utils.point__GT_neighbors.call(null,dimensions,m,new cljs.core.Keyword(null,"point","point",1813198264).cljs$core$IFn$_invoke$arity$1(tile),include_diagonals_QMARK_);
var shuffled_neighbors = cljs.core.shuffle.call(null,neighbors);
return cljs.core.map.call(null,f,shuffled_neighbors);
}));

(four_nations.model.map.utils.randomly_map_over_neighbors.cljs$lang$maxFixedArity = 5);

/**
 * Returns true if every neighbor of the tile satisfies the predicate.
 */
four_nations.model.map.utils.all_neighbors_are_QMARK_ = (function four_nations$model$map$utils$all_neighbors_are_QMARK_(var_args){
var G__39595 = arguments.length;
switch (G__39595) {
case 4:
return four_nations.model.map.utils.all_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
case 5:
return four_nations.model.map.utils.all_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$5((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]),(arguments[(4)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.utils.all_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$4 = (function (pred,tile,m,dimensions){
return four_nations.model.map.utils.all_neighbors_are_QMARK_.call(null,pred,tile,m,dimensions,true);
}));

(four_nations.model.map.utils.all_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$5 = (function (pred,tile,m,dimensions,include_diagonal_QMARK_){
return cljs.core.every_QMARK_.call(null,cljs.core.true_QMARK_,four_nations.model.map.utils.map_over_neighbors.call(null,pred,tile,m,dimensions,include_diagonal_QMARK_));
}));

(four_nations.model.map.utils.all_neighbors_are_QMARK_.cljs$lang$maxFixedArity = 5);

/**
 * Returns true if at least one neighbor of the tile satisfy the predicate.
 */
four_nations.model.map.utils.some_neighbors_are_QMARK_ = (function four_nations$model$map$utils$some_neighbors_are_QMARK_(var_args){
var G__39598 = arguments.length;
switch (G__39598) {
case 4:
return four_nations.model.map.utils.some_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
case 5:
return four_nations.model.map.utils.some_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$5((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]),(arguments[(4)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.utils.some_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$4 = (function (pred,tile,m,dimensions){
return four_nations.model.map.utils.some_neighbors_are_QMARK_.call(null,pred,tile,m,dimensions,true);
}));

(four_nations.model.map.utils.some_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$5 = (function (pred,tile,m,dimensions,include_diagonals_QMARK_){
return cljs.core.boolean$.call(null,cljs.core.some.call(null,cljs.core.true_QMARK_,four_nations.model.map.utils.map_over_neighbors.call(null,pred,tile,m,dimensions,include_diagonals_QMARK_)));
}));

(four_nations.model.map.utils.some_neighbors_are_QMARK_.cljs$lang$maxFixedArity = 5);

/**
 * Returns true if no neighbors of the tile satisfy the predicate.
 */
four_nations.model.map.utils.no_neighbors_are_QMARK_ = (function four_nations$model$map$utils$no_neighbors_are_QMARK_(var_args){
var G__39601 = arguments.length;
switch (G__39601) {
case 4:
return four_nations.model.map.utils.no_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
case 5:
return four_nations.model.map.utils.no_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$5((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]),(arguments[(4)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.utils.no_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$4 = (function (pred,tile,m,dimensions){
return four_nations.model.map.utils.no_neighbors_are_QMARK_.call(null,pred,tile,m,dimensions,true);
}));

(four_nations.model.map.utils.no_neighbors_are_QMARK_.cljs$core$IFn$_invoke$arity$5 = (function (pred,tile,m,dimensions,include_diagonals_QMARK_){
return (!(four_nations.model.map.utils.some_neighbors_are_QMARK_.call(null,pred,tile,m,dimensions,include_diagonals_QMARK_)));
}));

(four_nations.model.map.utils.no_neighbors_are_QMARK_.cljs$lang$maxFixedArity = 5);

/**
 * Returns true if the tile has the specified terrain type.
 */
four_nations.model.map.utils.has_terrain_type_QMARK_ = (function four_nations$model$map$utils$has_terrain_type_QMARK_(terrain_type,tile){
return cljs.core._EQ_.call(null,cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111)], null)),terrain_type);
});
four_nations.model.map.utils.terrain_type__GT_color = new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"land","land",-1402569867),"green",new cljs.core.Keyword(null,"water","water",-824098213),"blue",new cljs.core.Keyword(null,"coast","coast",1578056704),"yellow"], null);
four_nations.model.map.utils.terrain_type__GT_char = new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"land","land",-1402569867),"*",new cljs.core.Keyword(null,"water","water",-824098213),"~",new cljs.core.Keyword(null,"coast","coast",1578056704),"."], null);
four_nations.model.map.utils.tile__GT_printable_char = (function four_nations$model$map$utils$tile__GT_printable_char(tile){
var terrain_type = cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111)], null));
var resource = cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"resource","resource",251898836)], null));
var output_char = (function (){var or__4126__auto__ = new cljs.core.Keyword(null,"symbol","symbol",-1038572696).cljs$core$IFn$_invoke$arity$1(resource);
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
var or__4126__auto____$1 = four_nations.model.map.utils.terrain_type__GT_char.call(null,terrain_type);
if(cljs.core.truth_(or__4126__auto____$1)){
return or__4126__auto____$1;
} else {
return "?";
}
}
})();
return output_char;
});
/**
 * Given a game map, prints it out in a readable way.
 */
four_nations.model.map.utils.map__GT_string = (function four_nations$model$map$utils$map__GT_string(m,dimension){
return clojure.string.join.call(null,"\n",(function (){var iter__4529__auto__ = (function four_nations$model$map$utils$map__GT_string_$_iter__39603(s__39604){
return (new cljs.core.LazySeq(null,(function (){
var s__39604__$1 = s__39604;
while(true){
var temp__5720__auto__ = cljs.core.seq.call(null,s__39604__$1);
if(temp__5720__auto__){
var s__39604__$2 = temp__5720__auto__;
if(cljs.core.chunked_seq_QMARK_.call(null,s__39604__$2)){
var c__4527__auto__ = cljs.core.chunk_first.call(null,s__39604__$2);
var size__4528__auto__ = cljs.core.count.call(null,c__4527__auto__);
var b__39606 = cljs.core.chunk_buffer.call(null,size__4528__auto__);
if((function (){var i__39605 = (0);
while(true){
if((i__39605 < size__4528__auto__)){
var y = cljs.core._nth.call(null,c__4527__auto__,i__39605);
cljs.core.chunk_append.call(null,b__39606,clojure.string.join.call(null,(function (){var iter__4529__auto__ = ((function (i__39605,y,c__4527__auto__,size__4528__auto__,b__39606,s__39604__$2,temp__5720__auto__){
return (function four_nations$model$map$utils$map__GT_string_$_iter__39603_$_iter__39607(s__39608){
return (new cljs.core.LazySeq(null,((function (i__39605,y,c__4527__auto__,size__4528__auto__,b__39606,s__39604__$2,temp__5720__auto__){
return (function (){
var s__39608__$1 = s__39608;
while(true){
var temp__5720__auto____$1 = cljs.core.seq.call(null,s__39608__$1);
if(temp__5720__auto____$1){
var s__39608__$2 = temp__5720__auto____$1;
if(cljs.core.chunked_seq_QMARK_.call(null,s__39608__$2)){
var c__4527__auto____$1 = cljs.core.chunk_first.call(null,s__39608__$2);
var size__4528__auto____$1 = cljs.core.count.call(null,c__4527__auto____$1);
var b__39610 = cljs.core.chunk_buffer.call(null,size__4528__auto____$1);
if((function (){var i__39609 = (0);
while(true){
if((i__39609 < size__4528__auto____$1)){
var x = cljs.core._nth.call(null,c__4527__auto____$1,i__39609);
cljs.core.chunk_append.call(null,b__39610,four_nations.model.map.utils.tile__GT_printable_char.call(null,four_nations.model.map.utils.get_cell.call(null,m,four_nations.general.types.__GT_Point.call(null,x,y))));

var G__39615 = (i__39609 + (1));
i__39609 = G__39615;
continue;
} else {
return true;
}
break;
}
})()){
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__39610),four_nations$model$map$utils$map__GT_string_$_iter__39603_$_iter__39607.call(null,cljs.core.chunk_rest.call(null,s__39608__$2)));
} else {
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__39610),null);
}
} else {
var x = cljs.core.first.call(null,s__39608__$2);
return cljs.core.cons.call(null,four_nations.model.map.utils.tile__GT_printable_char.call(null,four_nations.model.map.utils.get_cell.call(null,m,four_nations.general.types.__GT_Point.call(null,x,y))),four_nations$model$map$utils$map__GT_string_$_iter__39603_$_iter__39607.call(null,cljs.core.rest.call(null,s__39608__$2)));
}
} else {
return null;
}
break;
}
});})(i__39605,y,c__4527__auto__,size__4528__auto__,b__39606,s__39604__$2,temp__5720__auto__))
,null,null));
});})(i__39605,y,c__4527__auto__,size__4528__auto__,b__39606,s__39604__$2,temp__5720__auto__))
;
return iter__4529__auto__.call(null,cljs.core.range.call(null,new cljs.core.Keyword(null,"width","width",-384071477).cljs$core$IFn$_invoke$arity$1(dimension)));
})()));

var G__39616 = (i__39605 + (1));
i__39605 = G__39616;
continue;
} else {
return true;
}
break;
}
})()){
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__39606),four_nations$model$map$utils$map__GT_string_$_iter__39603.call(null,cljs.core.chunk_rest.call(null,s__39604__$2)));
} else {
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__39606),null);
}
} else {
var y = cljs.core.first.call(null,s__39604__$2);
return cljs.core.cons.call(null,clojure.string.join.call(null,(function (){var iter__4529__auto__ = ((function (y,s__39604__$2,temp__5720__auto__){
return (function four_nations$model$map$utils$map__GT_string_$_iter__39603_$_iter__39611(s__39612){
return (new cljs.core.LazySeq(null,(function (){
var s__39612__$1 = s__39612;
while(true){
var temp__5720__auto____$1 = cljs.core.seq.call(null,s__39612__$1);
if(temp__5720__auto____$1){
var s__39612__$2 = temp__5720__auto____$1;
if(cljs.core.chunked_seq_QMARK_.call(null,s__39612__$2)){
var c__4527__auto__ = cljs.core.chunk_first.call(null,s__39612__$2);
var size__4528__auto__ = cljs.core.count.call(null,c__4527__auto__);
var b__39614 = cljs.core.chunk_buffer.call(null,size__4528__auto__);
if((function (){var i__39613 = (0);
while(true){
if((i__39613 < size__4528__auto__)){
var x = cljs.core._nth.call(null,c__4527__auto__,i__39613);
cljs.core.chunk_append.call(null,b__39614,four_nations.model.map.utils.tile__GT_printable_char.call(null,four_nations.model.map.utils.get_cell.call(null,m,four_nations.general.types.__GT_Point.call(null,x,y))));

var G__39617 = (i__39613 + (1));
i__39613 = G__39617;
continue;
} else {
return true;
}
break;
}
})()){
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__39614),four_nations$model$map$utils$map__GT_string_$_iter__39603_$_iter__39611.call(null,cljs.core.chunk_rest.call(null,s__39612__$2)));
} else {
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__39614),null);
}
} else {
var x = cljs.core.first.call(null,s__39612__$2);
return cljs.core.cons.call(null,four_nations.model.map.utils.tile__GT_printable_char.call(null,four_nations.model.map.utils.get_cell.call(null,m,four_nations.general.types.__GT_Point.call(null,x,y))),four_nations$model$map$utils$map__GT_string_$_iter__39603_$_iter__39611.call(null,cljs.core.rest.call(null,s__39612__$2)));
}
} else {
return null;
}
break;
}
}),null,null));
});})(y,s__39604__$2,temp__5720__auto__))
;
return iter__4529__auto__.call(null,cljs.core.range.call(null,new cljs.core.Keyword(null,"width","width",-384071477).cljs$core$IFn$_invoke$arity$1(dimension)));
})()),four_nations$model$map$utils$map__GT_string_$_iter__39603.call(null,cljs.core.rest.call(null,s__39604__$2)));
}
} else {
return null;
}
break;
}
}),null,null));
});
return iter__4529__auto__.call(null,cljs.core.range.call(null,new cljs.core.Keyword(null,"height","height",1025178622).cljs$core$IFn$_invoke$arity$1(dimension)));
})());
});

//# sourceMappingURL=utils.js.map?rel=1654487547479
