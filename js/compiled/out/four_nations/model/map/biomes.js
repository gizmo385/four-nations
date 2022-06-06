// Compiled by ClojureScript 1.10.773 {}
goog.provide('four_nations.model.map.biomes');
goog.require('cljs.core');
goog.require('four_nations.general.utils');
goog.require('four_nations.general.types');
goog.require('four_nations.model.map.utils');
/**
 * Generates the specified number of points within the given dimensions.
 */
four_nations.model.map.biomes.select_random_points = (function four_nations$model$map$biomes$select_random_points(dimensions,points_to_generate){
return cljs.core.repeatedly.call(null,points_to_generate,cljs.core.partial.call(null,four_nations.model.map.utils.random_point_in_dimensions,dimensions));
});
/**
 * Given a map m, determines if the tiles in that map have been fully-grouped by a group ID.
 */
four_nations.model.map.biomes.fully_grouped_QMARK_ = (function four_nations$model$map$biomes$fully_grouped_QMARK_(m){
return cljs.core.every_QMARK_.call(null,cljs.core.some_QMARK_,cljs.core.map.call(null,(function (p1__39693_SHARP_){
return cljs.core.get_in.call(null,p1__39693_SHARP_,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"group-id","group-id",-1316082778)], null));
}),cljs.core.vals.call(null,m)));
});
/**
 * Given a tile from a map, randomly selects one of the group IDs from that tiles neighbors.
 */
four_nations.model.map.biomes.select_group_id_from_neighbors = (function four_nations$model$map$biomes$select_group_id_from_neighbors(tile,m,dimensions){
var tile__GT_group_id = (function (t){
return cljs.core.get_in.call(null,t,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"group-id","group-id",-1316082778)], null));
});
var group_id = (function (){var G__39694 = four_nations.model.map.utils.map_over_neighbors.call(null,tile__GT_group_id,tile,m,dimensions);
var G__39694__$1 = (((G__39694 == null))?null:cljs.core.filter.call(null,cljs.core.some_QMARK_,G__39694));
var G__39694__$2 = (((G__39694__$1 == null))?null:cljs.core.not_empty.call(null,G__39694__$1));
if((G__39694__$2 == null)){
return null;
} else {
return cljs.core.rand_nth.call(null,G__39694__$2);
}
})();
return cljs.core.assoc_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"group-id","group-id",-1316082778)], null),group_id);
});
/**
 * Given map, spreads tile ids to neighbors.
 */
four_nations.model.map.biomes.spread_groups = (function four_nations$model$map$biomes$spread_groups(dimensions,m){
return four_nations.model.map.utils.randomly_map_over_tiles.call(null,(function (tile){
var G__39695 = tile;
if((cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"group-id","group-id",-1316082778)], null)) == null)){
return four_nations.model.map.biomes.select_group_id_from_neighbors.call(null,G__39695,m,dimensions);
} else {
return G__39695;
}
}),m,dimensions);
});
/**
 * Given a map and its dimensions, subdivides a map into the specified number of divisions, with each
 * division ID'd from 0 to the number of subgroup IDs specified. This means you could divide the map
 * into 10 different segments and each segment would be have an ID between [0, 4].
 */
four_nations.model.map.biomes.subdivide_map = (function four_nations$model$map$biomes$subdivide_map(m,dimensions,subgroup_count,subgroup_id_count){
var initial_points = four_nations.model.map.biomes.select_random_points.call(null,dimensions,subgroup_count);
var initial_tiling = cljs.core.reduce.call(null,(function (game_map,p__39696){
var vec__39697 = p__39696;
var idx = cljs.core.nth.call(null,vec__39697,(0),null);
var p = cljs.core.nth.call(null,vec__39697,(1),null);
return cljs.core.assoc_in.call(null,game_map,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [p,new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"group-id","group-id",-1316082778)], null),idx);
}),m,cljs.core.zipmap.call(null,cljs.core.repeatedly.call(null,(function (){
return (cljs.core.rand.call(null,subgroup_id_count) | (0));
})),initial_points));
return cljs.core.last.call(null,cljs.core.take_while.call(null,cljs.core.complement.call(null,four_nations.model.map.biomes.fully_grouped_QMARK_),cljs.core.iterate.call(null,cljs.core.partial.call(null,four_nations.model.map.biomes.spread_groups,dimensions),initial_tiling)));
});
/**
 * Replaces the group ID on each tile with the biome that group ID corresponds to.
 */
four_nations.model.map.biomes.group_ids__GT_biomes = (function four_nations$model$map$biomes$group_ids__GT_biomes(m,dimensions,id__GT_biome){
return four_nations.model.map.utils.map_over_tiles.call(null,(function (tile){
var biome = id__GT_biome.call(null,cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"group-id","group-id",-1316082778)], null)));
return cljs.core.update.call(null,cljs.core.assoc_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"biome","biome",1856013988)], null),biome),new cljs.core.Keyword(null,"attributes","attributes",-74013604),cljs.core.dissoc,new cljs.core.Keyword(null,"group-id","group-id",-1316082778));
}),m,dimensions);
});
/**
 * Given a map and its dimensions, divides the map and adds biomes from a defined set of available
 * biomes.
 */
four_nations.model.map.biomes.add_biomes = (function four_nations$model$map$biomes$add_biomes(m,dimensions,biome_count,biomes){
var id__GT_biome = cljs.core.zipmap.call(null,cljs.core.range.call(null),biomes);
return four_nations.model.map.biomes.group_ids__GT_biomes.call(null,four_nations.model.map.biomes.subdivide_map.call(null,cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,m),dimensions,biome_count,cljs.core.count.call(null,biomes)),dimensions,id__GT_biome);
});

/**
* @constructor
 * @implements {cljs.core.IRecord}
 * @implements {cljs.core.IKVReduce}
 * @implements {cljs.core.IEquiv}
 * @implements {cljs.core.IHash}
 * @implements {cljs.core.ICollection}
 * @implements {cljs.core.ICounted}
 * @implements {cljs.core.ISeqable}
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.ICloneable}
 * @implements {cljs.core.IPrintWithWriter}
 * @implements {cljs.core.IIterable}
 * @implements {cljs.core.IWithMeta}
 * @implements {cljs.core.IAssociative}
 * @implements {cljs.core.IMap}
 * @implements {cljs.core.ILookup}
*/
four_nations.model.map.biomes.Biome = (function (name,__meta,__extmap,__hash){
this.name = name;
this.__meta = __meta;
this.__extmap = __extmap;
this.__hash = __hash;
this.cljs$lang$protocol_mask$partition0$ = 2230716170;
this.cljs$lang$protocol_mask$partition1$ = 139264;
});
(four_nations.model.map.biomes.Biome.prototype.cljs$core$ILookup$_lookup$arity$2 = (function (this__4380__auto__,k__4381__auto__){
var self__ = this;
var this__4380__auto____$1 = this;
return this__4380__auto____$1.cljs$core$ILookup$_lookup$arity$3(null,k__4381__auto__,null);
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$ILookup$_lookup$arity$3 = (function (this__4382__auto__,k39701,else__4383__auto__){
var self__ = this;
var this__4382__auto____$1 = this;
var G__39705 = k39701;
var G__39705__$1 = (((G__39705 instanceof cljs.core.Keyword))?G__39705.fqn:null);
switch (G__39705__$1) {
case "name":
return self__.name;

break;
default:
return cljs.core.get.call(null,self__.__extmap,k39701,else__4383__auto__);

}
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$IKVReduce$_kv_reduce$arity$3 = (function (this__4399__auto__,f__4400__auto__,init__4401__auto__){
var self__ = this;
var this__4399__auto____$1 = this;
return cljs.core.reduce.call(null,(function (ret__4402__auto__,p__39706){
var vec__39707 = p__39706;
var k__4403__auto__ = cljs.core.nth.call(null,vec__39707,(0),null);
var v__4404__auto__ = cljs.core.nth.call(null,vec__39707,(1),null);
return f__4400__auto__.call(null,ret__4402__auto__,k__4403__auto__,v__4404__auto__);
}),init__4401__auto__,this__4399__auto____$1);
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$IPrintWithWriter$_pr_writer$arity$3 = (function (this__4394__auto__,writer__4395__auto__,opts__4396__auto__){
var self__ = this;
var this__4394__auto____$1 = this;
var pr_pair__4397__auto__ = (function (keyval__4398__auto__){
return cljs.core.pr_sequential_writer.call(null,writer__4395__auto__,cljs.core.pr_writer,""," ","",opts__4396__auto__,keyval__4398__auto__);
});
return cljs.core.pr_sequential_writer.call(null,writer__4395__auto__,pr_pair__4397__auto__,"#four-nations.model.map.biomes.Biome{",", ","}",opts__4396__auto__,cljs.core.concat.call(null,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"name","name",1843675177),self__.name],null))], null),self__.__extmap));
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$IIterable$_iterator$arity$1 = (function (G__39700){
var self__ = this;
var G__39700__$1 = this;
return (new cljs.core.RecordIter((0),G__39700__$1,1,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"name","name",1843675177)], null),(cljs.core.truth_(self__.__extmap)?cljs.core._iterator.call(null,self__.__extmap):cljs.core.nil_iter.call(null))));
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$IMeta$_meta$arity$1 = (function (this__4378__auto__){
var self__ = this;
var this__4378__auto____$1 = this;
return self__.__meta;
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$ICloneable$_clone$arity$1 = (function (this__4375__auto__){
var self__ = this;
var this__4375__auto____$1 = this;
return (new four_nations.model.map.biomes.Biome(self__.name,self__.__meta,self__.__extmap,self__.__hash));
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$ICounted$_count$arity$1 = (function (this__4384__auto__){
var self__ = this;
var this__4384__auto____$1 = this;
return (1 + cljs.core.count.call(null,self__.__extmap));
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$IHash$_hash$arity$1 = (function (this__4376__auto__){
var self__ = this;
var this__4376__auto____$1 = this;
var h__4238__auto__ = self__.__hash;
if((!((h__4238__auto__ == null)))){
return h__4238__auto__;
} else {
var h__4238__auto____$1 = (function (coll__4377__auto__){
return (-369771725 ^ cljs.core.hash_unordered_coll.call(null,coll__4377__auto__));
}).call(null,this__4376__auto____$1);
(self__.__hash = h__4238__auto____$1);

return h__4238__auto____$1;
}
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$IEquiv$_equiv$arity$2 = (function (this39702,other39703){
var self__ = this;
var this39702__$1 = this;
return (((!((other39703 == null)))) && ((this39702__$1.constructor === other39703.constructor)) && (cljs.core._EQ_.call(null,this39702__$1.name,other39703.name)) && (cljs.core._EQ_.call(null,this39702__$1.__extmap,other39703.__extmap)));
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$IMap$_dissoc$arity$2 = (function (this__4389__auto__,k__4390__auto__){
var self__ = this;
var this__4389__auto____$1 = this;
if(cljs.core.contains_QMARK_.call(null,new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"name","name",1843675177),null], null), null),k__4390__auto__)){
return cljs.core.dissoc.call(null,cljs.core._with_meta.call(null,cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,this__4389__auto____$1),self__.__meta),k__4390__auto__);
} else {
return (new four_nations.model.map.biomes.Biome(self__.name,self__.__meta,cljs.core.not_empty.call(null,cljs.core.dissoc.call(null,self__.__extmap,k__4390__auto__)),null));
}
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$IAssociative$_assoc$arity$3 = (function (this__4387__auto__,k__4388__auto__,G__39700){
var self__ = this;
var this__4387__auto____$1 = this;
var pred__39710 = cljs.core.keyword_identical_QMARK_;
var expr__39711 = k__4388__auto__;
if(cljs.core.truth_(pred__39710.call(null,new cljs.core.Keyword(null,"name","name",1843675177),expr__39711))){
return (new four_nations.model.map.biomes.Biome(G__39700,self__.__meta,self__.__extmap,null));
} else {
return (new four_nations.model.map.biomes.Biome(self__.name,self__.__meta,cljs.core.assoc.call(null,self__.__extmap,k__4388__auto__,G__39700),null));
}
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$ISeqable$_seq$arity$1 = (function (this__4392__auto__){
var self__ = this;
var this__4392__auto____$1 = this;
return cljs.core.seq.call(null,cljs.core.concat.call(null,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [(new cljs.core.MapEntry(new cljs.core.Keyword(null,"name","name",1843675177),self__.name,null))], null),self__.__extmap));
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (this__4379__auto__,G__39700){
var self__ = this;
var this__4379__auto____$1 = this;
return (new four_nations.model.map.biomes.Biome(self__.name,G__39700,self__.__extmap,self__.__hash));
}));

(four_nations.model.map.biomes.Biome.prototype.cljs$core$ICollection$_conj$arity$2 = (function (this__4385__auto__,entry__4386__auto__){
var self__ = this;
var this__4385__auto____$1 = this;
if(cljs.core.vector_QMARK_.call(null,entry__4386__auto__)){
return this__4385__auto____$1.cljs$core$IAssociative$_assoc$arity$3(null,cljs.core._nth.call(null,entry__4386__auto__,(0)),cljs.core._nth.call(null,entry__4386__auto__,(1)));
} else {
return cljs.core.reduce.call(null,cljs.core._conj,this__4385__auto____$1,entry__4386__auto__);
}
}));

(four_nations.model.map.biomes.Biome.getBasis = (function (){
return new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"name","name",-810760592,null)], null);
}));

(four_nations.model.map.biomes.Biome.cljs$lang$type = true);

(four_nations.model.map.biomes.Biome.cljs$lang$ctorPrSeq = (function (this__4423__auto__){
return (new cljs.core.List(null,"four-nations.model.map.biomes/Biome",null,(1),null));
}));

(four_nations.model.map.biomes.Biome.cljs$lang$ctorPrWriter = (function (this__4423__auto__,writer__4424__auto__){
return cljs.core._write.call(null,writer__4424__auto__,"four-nations.model.map.biomes/Biome");
}));

/**
 * Positional factory function for four-nations.model.map.biomes/Biome.
 */
four_nations.model.map.biomes.__GT_Biome = (function four_nations$model$map$biomes$__GT_Biome(name){
return (new four_nations.model.map.biomes.Biome(name,null,null,null));
});

/**
 * Factory function for four-nations.model.map.biomes/Biome, taking a map of keywords to field values.
 */
four_nations.model.map.biomes.map__GT_Biome = (function four_nations$model$map$biomes$map__GT_Biome(G__39704){
var extmap__4419__auto__ = (function (){var G__39713 = cljs.core.dissoc.call(null,G__39704,new cljs.core.Keyword(null,"name","name",1843675177));
if(cljs.core.record_QMARK_.call(null,G__39704)){
return cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,G__39713);
} else {
return G__39713;
}
})();
return (new four_nations.model.map.biomes.Biome(new cljs.core.Keyword(null,"name","name",1843675177).cljs$core$IFn$_invoke$arity$1(G__39704),null,cljs.core.not_empty.call(null,extmap__4419__auto__),null));
});


//# sourceMappingURL=biomes.js.map?rel=1654487547708
