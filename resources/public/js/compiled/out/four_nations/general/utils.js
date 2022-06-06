// Compiled by ClojureScript 1.10.773 {}
goog.provide('four_nations.general.utils');
goog.require('cljs.core');
goog.require('clojure.edn');
/**
 * Read the file at the specified path
 */
four_nations.general.utils.read_file = (function four_nations$general$utils$read_file(path){
return four_nations.general.utils.fs.readFile(path,"utf8");
});
/**
 * Given a function f, an initial value, and a number n, performs n calls of f. The initial value is
 */
four_nations.general.utils.n_times = (function four_nations$general$utils$n_times(f,init,n){
return cljs.core.reduce.call(null,(function (value,_idx){
return f.call(null,value);
}),init,cljs.core.range.call(null,n));
});
/**
 * Given a key k and collection of maps, returns a map that maps the value of k in each item in the
 * collection to that item.
 */
four_nations.general.utils.map_by = (function four_nations$general$utils$map_by(k,coll){
return cljs.core.zipmap.call(null,cljs.core.map.call(null,k,coll),coll);
});
/**
 * Given a function f and a map m, returns a new map where f has been applied to each value in m.
 */
four_nations.general.utils.map_vals = (function four_nations$general$utils$map_vals(f,m){
return cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,(function (){var iter__4529__auto__ = (function four_nations$general$utils$map_vals_$_iter__39544(s__39545){
return (new cljs.core.LazySeq(null,(function (){
var s__39545__$1 = s__39545;
while(true){
var temp__5720__auto__ = cljs.core.seq.call(null,s__39545__$1);
if(temp__5720__auto__){
var s__39545__$2 = temp__5720__auto__;
if(cljs.core.chunked_seq_QMARK_.call(null,s__39545__$2)){
var c__4527__auto__ = cljs.core.chunk_first.call(null,s__39545__$2);
var size__4528__auto__ = cljs.core.count.call(null,c__4527__auto__);
var b__39547 = cljs.core.chunk_buffer.call(null,size__4528__auto__);
if((function (){var i__39546 = (0);
while(true){
if((i__39546 < size__4528__auto__)){
var vec__39548 = cljs.core._nth.call(null,c__4527__auto__,i__39546);
var k = cljs.core.nth.call(null,vec__39548,(0),null);
var v = cljs.core.nth.call(null,vec__39548,(1),null);
cljs.core.chunk_append.call(null,b__39547,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [k,f.call(null,v)], null));

var G__39554 = (i__39546 + (1));
i__39546 = G__39554;
continue;
} else {
return true;
}
break;
}
})()){
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__39547),four_nations$general$utils$map_vals_$_iter__39544.call(null,cljs.core.chunk_rest.call(null,s__39545__$2)));
} else {
return cljs.core.chunk_cons.call(null,cljs.core.chunk.call(null,b__39547),null);
}
} else {
var vec__39551 = cljs.core.first.call(null,s__39545__$2);
var k = cljs.core.nth.call(null,vec__39551,(0),null);
var v = cljs.core.nth.call(null,vec__39551,(1),null);
return cljs.core.cons.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [k,f.call(null,v)], null),four_nations$general$utils$map_vals_$_iter__39544.call(null,cljs.core.rest.call(null,s__39545__$2)));
}
} else {
return null;
}
break;
}
}),null,null));
});
return iter__4529__auto__.call(null,m);
})());
});

//# sourceMappingURL=utils.js.map?rel=1654487547422
