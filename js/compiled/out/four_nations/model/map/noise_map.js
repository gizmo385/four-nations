// Compiled by ClojureScript 1.10.773 {}
goog.provide('four_nations.model.map.noise_map');
goog.require('cljs.core');
goog.require('four_nations.general.types');
goog.require('four_nations.general.utils');
goog.require('four_nations.model.map.utils');
/**
 * Given a specified dimension, generates a mapping from coordinates to map tiles with that
 * dimension.
 */
four_nations.model.map.noise_map.generate_points = (function four_nations$model$map$noise_map$generate_points(var_args){
var G__39621 = arguments.length;
switch (G__39621) {
case 1:
return four_nations.model.map.noise_map.generate_points.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 2:
return four_nations.model.map.noise_map.generate_points.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.noise_map.generate_points.cljs$core$IFn$_invoke$arity$1 = (function (dim){
return four_nations.model.map.noise_map.generate_points.call(null,dim,cljs.core.constantly.call(null,(0)));
}));

(four_nations.model.map.noise_map.generate_points.cljs$core$IFn$_invoke$arity$2 = (function (dim,value_fn){
return cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,(function (){var iter__4529__auto__ = (function four_nations$model$map$noise_map$iter__39622(s__39623){
return (new cljs.core.LazySeq(null,(function (){
var s__39623__$1 = s__39623;
while(true){
var temp__5720__auto__ = cljs.core.seq.call(null,s__39623__$1);
if(temp__5720__auto__){
var xs__6277__auto__ = temp__5720__auto__;
var x = cljs.core.first.call(null,xs__6277__auto__);
var iterys__4525__auto__ = ((function (s__39623__$1,x,xs__6277__auto__,temp__5720__auto__){
return (function four_nations$model$map$noise_map$iter__39622_$_iter__39624(s__39625){
return (new cljs.core.LazySeq(null,((function (s__39623__$1,x,xs__6277__auto__,temp__5720__auto__){
return (function (){
var s__39625__$1 = s__39625;
while(true){
var temp__5720__auto____$1 = cljs.core.seq.call(null,s__39625__$1);
if(temp__5720__auto____$1){
var s__39625__$2 = temp__5720__auto____$1;
if(cljs.core.chunked_seq_QMARK_.call(null,s__39625__$2)){
var c__4527__auto__ = cljs.core.chunk_first.call(null,s__39625__$2);
var size__4528__auto__ = cljs.core.count.call(null,c__4527__auto__);
var b__39627 = cljs.core.chunk_buffer.call(null,size__4528__auto__);
if((function (){var i__39626 = (0);
while(true){
if((i__39626 < size__4528__auto__)){
var y = cljs.core._nth.call(null,c__4527__auto__,i__39626);
var point = four_nations.general.types.__GT_Point.call(null,x,y);
var tile = four_nations.general.types.__GT_MapTile.call(null,point,value_fn.call(null),cljs.core.PersistentArrayMap.EMPTY);
cljs.core.chunk_append.call(null,b__39627,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [point,tile], null));

var G__39629 = (i__39626 + (1));
i__39626 = G__39629;
continue;
} else {
return true;
}
break;
}
})()){
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__39627),four_nations$model$map$noise_map$iter__39622_$_iter__39624.call(null,cljs.core.chunk_rest.call(null,s__39625__$2)));
} else {
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__39627),null);
}
} else {
var y = cljs.core.first.call(null,s__39625__$2);
var point = four_nations.general.types.__GT_Point.call(null,x,y);
var tile = four_nations.general.types.__GT_MapTile.call(null,point,value_fn.call(null),cljs.core.PersistentArrayMap.EMPTY);
return cljs.core.cons.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [point,tile], null),four_nations$model$map$noise_map$iter__39622_$_iter__39624.call(null,cljs.core.rest.call(null,s__39625__$2)));
}
} else {
return null;
}
break;
}
});})(s__39623__$1,x,xs__6277__auto__,temp__5720__auto__))
,null,null));
});})(s__39623__$1,x,xs__6277__auto__,temp__5720__auto__))
;
var fs__4526__auto__ = cljs.core.seq.call(null,iterys__4525__auto__.call(null,cljs.core.range.call(null,new cljs.core.Keyword(null,"height","height",1025178622).cljs$core$IFn$_invoke$arity$1(dim))));
if(fs__4526__auto__){
return cljs.core.concat.call(null,fs__4526__auto__,four_nations$model$map$noise_map$iter__39622.call(null,cljs.core.rest.call(null,s__39623__$1)));
} else {
var G__39630 = cljs.core.rest.call(null,s__39623__$1);
s__39623__$1 = G__39630;
continue;
}
} else {
return null;
}
break;
}
}),null,null));
});
return iter__4529__auto__.call(null,cljs.core.range.call(null,new cljs.core.Keyword(null,"width","width",-384071477).cljs$core$IFn$_invoke$arity$1(dim)));
})());
}));

(four_nations.model.map.noise_map.generate_points.cljs$lang$maxFixedArity = 2);

/**
 * Given a height and a width, generates a two-dimensional array of noise, where each cell is
 * occupied by a number between 1 and 255 (inclusive on both sides).
 */
four_nations.model.map.noise_map.generate_2d_noise = (function four_nations$model$map$noise_map$generate_2d_noise(dim){
var cell_value = (function four_nations$model$map$noise_map$generate_2d_noise_$_cell_value(){
return (cljs.core.rand_int.call(null,(255)) + (1));
});
return four_nations.model.map.noise_map.generate_points.call(null,dim,cell_value);
});
/**
 * Given a 2-dimensional array of noise, a height, width, and a cell coordinate, calculates the
 * average of the values of its neighbor cells
 */
four_nations.model.map.noise_map.average_of_neighbors = (function four_nations$model$map$noise_map$average_of_neighbors(noise,dimension,point){
var neighbors = four_nations.model.map.utils.point__GT_neighbors.call(null,dimension,noise,point);
var neighbor_values = cljs.core.map.call(null,new cljs.core.Keyword(null,"value","value",305978217),neighbors);
return cljs.core.quot.call(null,cljs.core.apply.call(null,cljs.core._PLUS_,neighbor_values),cljs.core.count.call(null,neighbor_values));
});
/**
 * Given a 2-dimensional array of noise, returns a 2-dimensional array of averaged noise, where the
 * value of each cell is the average of the values of its neighbors.
 */
four_nations.model.map.noise_map.average_2d_noise = (function four_nations$model$map$noise_map$average_2d_noise(dimension,noise){
return cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,cljs.core.map.call(null,(function (p__39631){
var vec__39632 = p__39631;
var point = cljs.core.nth.call(null,vec__39632,(0),null);
var tile = cljs.core.nth.call(null,vec__39632,(1),null);
return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [point,cljs.core.assoc.call(null,tile,new cljs.core.Keyword(null,"value","value",305978217),four_nations.model.map.noise_map.average_of_neighbors.call(null,noise,dimension,point))], null);
}),noise));
});
/**
 * Given a smooth map, smoothes that map by averaging cell values with neighbors the specified number
 * of times.
 */
four_nations.model.map.noise_map.smooth_noisemap = (function four_nations$model$map$noise_map$smooth_noisemap(noise,dimension,smoothing_passes){
return cljs.core.nth.call(null,cljs.core.iterate.call(null,cljs.core.partial.call(null,four_nations.model.map.noise_map.average_2d_noise,dimension),noise),smoothing_passes);
});
/**
 * Given a dimension, generates a 2-dimensional map of averaged noise, which can be used for
 * procedural terrain generation. You can optionally supply the number of times that the noise will
 * be smoothed out (defaults to only a single smoothing pass).
 */
four_nations.model.map.noise_map.generate_noisemap = (function four_nations$model$map$noise_map$generate_noisemap(var_args){
var G__39636 = arguments.length;
switch (G__39636) {
case 1:
return four_nations.model.map.noise_map.generate_noisemap.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 2:
return four_nations.model.map.noise_map.generate_noisemap.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(four_nations.model.map.noise_map.generate_noisemap.cljs$core$IFn$_invoke$arity$1 = (function (dimension){
return four_nations.model.map.noise_map.generate_noisemap.call(null,dimension,(1));
}));

(four_nations.model.map.noise_map.generate_noisemap.cljs$core$IFn$_invoke$arity$2 = (function (dimension,smoothing_passes){
return four_nations.model.map.noise_map.smooth_noisemap.call(null,four_nations.model.map.noise_map.generate_2d_noise.call(null,dimension),dimension,smoothing_passes);
}));

(four_nations.model.map.noise_map.generate_noisemap.cljs$lang$maxFixedArity = 2);


//# sourceMappingURL=noise_map.js.map?rel=1654487547522
