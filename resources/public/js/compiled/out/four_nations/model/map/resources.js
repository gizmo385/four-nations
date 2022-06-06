// Compiled by ClojureScript 1.10.773 {}
goog.provide('four_nations.model.map.resources');
goog.require('cljs.core');
goog.require('four_nations.general.utils');

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
four_nations.model.map.resources.Resource = (function (name,display_name,text_symbol,spawn_predicate,image,__meta,__extmap,__hash){
this.name = name;
this.display_name = display_name;
this.text_symbol = text_symbol;
this.spawn_predicate = spawn_predicate;
this.image = image;
this.__meta = __meta;
this.__extmap = __extmap;
this.__hash = __hash;
this.cljs$lang$protocol_mask$partition0$ = 2230716170;
this.cljs$lang$protocol_mask$partition1$ = 139264;
});
(four_nations.model.map.resources.Resource.prototype.cljs$core$ILookup$_lookup$arity$2 = (function (this__4380__auto__,k__4381__auto__){
var self__ = this;
var this__4380__auto____$1 = this;
return this__4380__auto____$1.cljs$core$ILookup$_lookup$arity$3(null,k__4381__auto__,null);
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$ILookup$_lookup$arity$3 = (function (this__4382__auto__,k39677,else__4383__auto__){
var self__ = this;
var this__4382__auto____$1 = this;
var G__39681 = k39677;
var G__39681__$1 = (((G__39681 instanceof cljs.core.Keyword))?G__39681.fqn:null);
switch (G__39681__$1) {
case "name":
return self__.name;

break;
case "display-name":
return self__.display_name;

break;
case "text-symbol":
return self__.text_symbol;

break;
case "spawn-predicate":
return self__.spawn_predicate;

break;
case "image":
return self__.image;

break;
default:
return cljs.core.get.call(null,self__.__extmap,k39677,else__4383__auto__);

}
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$IKVReduce$_kv_reduce$arity$3 = (function (this__4399__auto__,f__4400__auto__,init__4401__auto__){
var self__ = this;
var this__4399__auto____$1 = this;
return cljs.core.reduce.call(null,(function (ret__4402__auto__,p__39682){
var vec__39683 = p__39682;
var k__4403__auto__ = cljs.core.nth.call(null,vec__39683,(0),null);
var v__4404__auto__ = cljs.core.nth.call(null,vec__39683,(1),null);
return f__4400__auto__.call(null,ret__4402__auto__,k__4403__auto__,v__4404__auto__);
}),init__4401__auto__,this__4399__auto____$1);
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$IPrintWithWriter$_pr_writer$arity$3 = (function (this__4394__auto__,writer__4395__auto__,opts__4396__auto__){
var self__ = this;
var this__4394__auto____$1 = this;
var pr_pair__4397__auto__ = (function (keyval__4398__auto__){
return cljs.core.pr_sequential_writer.call(null,writer__4395__auto__,cljs.core.pr_writer,""," ","",opts__4396__auto__,keyval__4398__auto__);
});
return cljs.core.pr_sequential_writer.call(null,writer__4395__auto__,pr_pair__4397__auto__,"#four-nations.model.map.resources.Resource{",", ","}",opts__4396__auto__,cljs.core.concat.call(null,new cljs.core.PersistentVector(null, 5, 5, cljs.core.PersistentVector.EMPTY_NODE, [(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"name","name",1843675177),self__.name],null)),(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"display-name","display-name",694513143),self__.display_name],null)),(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"text-symbol","text-symbol",1309607671),self__.text_symbol],null)),(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"spawn-predicate","spawn-predicate",543545894),self__.spawn_predicate],null)),(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"image","image",-58725096),self__.image],null))], null),self__.__extmap));
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$IIterable$_iterator$arity$1 = (function (G__39676){
var self__ = this;
var G__39676__$1 = this;
return (new cljs.core.RecordIter((0),G__39676__$1,5,new cljs.core.PersistentVector(null, 5, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"display-name","display-name",694513143),new cljs.core.Keyword(null,"text-symbol","text-symbol",1309607671),new cljs.core.Keyword(null,"spawn-predicate","spawn-predicate",543545894),new cljs.core.Keyword(null,"image","image",-58725096)], null),(cljs.core.truth_(self__.__extmap)?cljs.core._iterator.call(null,self__.__extmap):cljs.core.nil_iter.call(null))));
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$IMeta$_meta$arity$1 = (function (this__4378__auto__){
var self__ = this;
var this__4378__auto____$1 = this;
return self__.__meta;
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$ICloneable$_clone$arity$1 = (function (this__4375__auto__){
var self__ = this;
var this__4375__auto____$1 = this;
return (new four_nations.model.map.resources.Resource(self__.name,self__.display_name,self__.text_symbol,self__.spawn_predicate,self__.image,self__.__meta,self__.__extmap,self__.__hash));
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$ICounted$_count$arity$1 = (function (this__4384__auto__){
var self__ = this;
var this__4384__auto____$1 = this;
return (5 + cljs.core.count.call(null,self__.__extmap));
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$IHash$_hash$arity$1 = (function (this__4376__auto__){
var self__ = this;
var this__4376__auto____$1 = this;
var h__4238__auto__ = self__.__hash;
if((!((h__4238__auto__ == null)))){
return h__4238__auto__;
} else {
var h__4238__auto____$1 = (function (coll__4377__auto__){
return (877302057 ^ cljs.core.hash_unordered_coll.call(null,coll__4377__auto__));
}).call(null,this__4376__auto____$1);
(self__.__hash = h__4238__auto____$1);

return h__4238__auto____$1;
}
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$IEquiv$_equiv$arity$2 = (function (this39678,other39679){
var self__ = this;
var this39678__$1 = this;
return (((!((other39679 == null)))) && ((this39678__$1.constructor === other39679.constructor)) && (cljs.core._EQ_.call(null,this39678__$1.name,other39679.name)) && (cljs.core._EQ_.call(null,this39678__$1.display_name,other39679.display_name)) && (cljs.core._EQ_.call(null,this39678__$1.text_symbol,other39679.text_symbol)) && (cljs.core._EQ_.call(null,this39678__$1.spawn_predicate,other39679.spawn_predicate)) && (cljs.core._EQ_.call(null,this39678__$1.image,other39679.image)) && (cljs.core._EQ_.call(null,this39678__$1.__extmap,other39679.__extmap)));
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$IMap$_dissoc$arity$2 = (function (this__4389__auto__,k__4390__auto__){
var self__ = this;
var this__4389__auto____$1 = this;
if(cljs.core.contains_QMARK_.call(null,new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 5, [new cljs.core.Keyword(null,"spawn-predicate","spawn-predicate",543545894),null,new cljs.core.Keyword(null,"name","name",1843675177),null,new cljs.core.Keyword(null,"display-name","display-name",694513143),null,new cljs.core.Keyword(null,"text-symbol","text-symbol",1309607671),null,new cljs.core.Keyword(null,"image","image",-58725096),null], null), null),k__4390__auto__)){
return cljs.core.dissoc.call(null,cljs.core._with_meta.call(null,cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,this__4389__auto____$1),self__.__meta),k__4390__auto__);
} else {
return (new four_nations.model.map.resources.Resource(self__.name,self__.display_name,self__.text_symbol,self__.spawn_predicate,self__.image,self__.__meta,cljs.core.not_empty.call(null,cljs.core.dissoc.call(null,self__.__extmap,k__4390__auto__)),null));
}
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$IAssociative$_assoc$arity$3 = (function (this__4387__auto__,k__4388__auto__,G__39676){
var self__ = this;
var this__4387__auto____$1 = this;
var pred__39686 = cljs.core.keyword_identical_QMARK_;
var expr__39687 = k__4388__auto__;
if(cljs.core.truth_(pred__39686.call(null,new cljs.core.Keyword(null,"name","name",1843675177),expr__39687))){
return (new four_nations.model.map.resources.Resource(G__39676,self__.display_name,self__.text_symbol,self__.spawn_predicate,self__.image,self__.__meta,self__.__extmap,null));
} else {
if(cljs.core.truth_(pred__39686.call(null,new cljs.core.Keyword(null,"display-name","display-name",694513143),expr__39687))){
return (new four_nations.model.map.resources.Resource(self__.name,G__39676,self__.text_symbol,self__.spawn_predicate,self__.image,self__.__meta,self__.__extmap,null));
} else {
if(cljs.core.truth_(pred__39686.call(null,new cljs.core.Keyword(null,"text-symbol","text-symbol",1309607671),expr__39687))){
return (new four_nations.model.map.resources.Resource(self__.name,self__.display_name,G__39676,self__.spawn_predicate,self__.image,self__.__meta,self__.__extmap,null));
} else {
if(cljs.core.truth_(pred__39686.call(null,new cljs.core.Keyword(null,"spawn-predicate","spawn-predicate",543545894),expr__39687))){
return (new four_nations.model.map.resources.Resource(self__.name,self__.display_name,self__.text_symbol,G__39676,self__.image,self__.__meta,self__.__extmap,null));
} else {
if(cljs.core.truth_(pred__39686.call(null,new cljs.core.Keyword(null,"image","image",-58725096),expr__39687))){
return (new four_nations.model.map.resources.Resource(self__.name,self__.display_name,self__.text_symbol,self__.spawn_predicate,G__39676,self__.__meta,self__.__extmap,null));
} else {
return (new four_nations.model.map.resources.Resource(self__.name,self__.display_name,self__.text_symbol,self__.spawn_predicate,self__.image,self__.__meta,cljs.core.assoc.call(null,self__.__extmap,k__4388__auto__,G__39676),null));
}
}
}
}
}
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$ISeqable$_seq$arity$1 = (function (this__4392__auto__){
var self__ = this;
var this__4392__auto____$1 = this;
return cljs.core.seq.call(null,cljs.core.concat.call(null,new cljs.core.PersistentVector(null, 5, 5, cljs.core.PersistentVector.EMPTY_NODE, [(new cljs.core.MapEntry(new cljs.core.Keyword(null,"name","name",1843675177),self__.name,null)),(new cljs.core.MapEntry(new cljs.core.Keyword(null,"display-name","display-name",694513143),self__.display_name,null)),(new cljs.core.MapEntry(new cljs.core.Keyword(null,"text-symbol","text-symbol",1309607671),self__.text_symbol,null)),(new cljs.core.MapEntry(new cljs.core.Keyword(null,"spawn-predicate","spawn-predicate",543545894),self__.spawn_predicate,null)),(new cljs.core.MapEntry(new cljs.core.Keyword(null,"image","image",-58725096),self__.image,null))], null),self__.__extmap));
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (this__4379__auto__,G__39676){
var self__ = this;
var this__4379__auto____$1 = this;
return (new four_nations.model.map.resources.Resource(self__.name,self__.display_name,self__.text_symbol,self__.spawn_predicate,self__.image,G__39676,self__.__extmap,self__.__hash));
}));

(four_nations.model.map.resources.Resource.prototype.cljs$core$ICollection$_conj$arity$2 = (function (this__4385__auto__,entry__4386__auto__){
var self__ = this;
var this__4385__auto____$1 = this;
if(cljs.core.vector_QMARK_.call(null,entry__4386__auto__)){
return this__4385__auto____$1.cljs$core$IAssociative$_assoc$arity$3(null,cljs.core._nth.call(null,entry__4386__auto__,(0)),cljs.core._nth.call(null,entry__4386__auto__,(1)));
} else {
return cljs.core.reduce.call(null,cljs.core._conj,this__4385__auto____$1,entry__4386__auto__);
}
}));

(four_nations.model.map.resources.Resource.getBasis = (function (){
return new cljs.core.PersistentVector(null, 5, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"name","name",-810760592,null),new cljs.core.Symbol(null,"display-name","display-name",-1959922626,null),new cljs.core.Symbol(null,"text-symbol","text-symbol",-1344828098,null),new cljs.core.Symbol(null,"spawn-predicate","spawn-predicate",-2110889875,null),new cljs.core.Symbol(null,"image","image",1581806431,null)], null);
}));

(four_nations.model.map.resources.Resource.cljs$lang$type = true);

(four_nations.model.map.resources.Resource.cljs$lang$ctorPrSeq = (function (this__4423__auto__){
return (new cljs.core.List(null,"four-nations.model.map.resources/Resource",null,(1),null));
}));

(four_nations.model.map.resources.Resource.cljs$lang$ctorPrWriter = (function (this__4423__auto__,writer__4424__auto__){
return cljs.core._write.call(null,writer__4424__auto__,"four-nations.model.map.resources/Resource");
}));

/**
 * Positional factory function for four-nations.model.map.resources/Resource.
 */
four_nations.model.map.resources.__GT_Resource = (function four_nations$model$map$resources$__GT_Resource(name,display_name,text_symbol,spawn_predicate,image){
return (new four_nations.model.map.resources.Resource(name,display_name,text_symbol,spawn_predicate,image,null,null,null));
});

/**
 * Factory function for four-nations.model.map.resources/Resource, taking a map of keywords to field values.
 */
four_nations.model.map.resources.map__GT_Resource = (function four_nations$model$map$resources$map__GT_Resource(G__39680){
var extmap__4419__auto__ = (function (){var G__39689 = cljs.core.dissoc.call(null,G__39680,new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"display-name","display-name",694513143),new cljs.core.Keyword(null,"text-symbol","text-symbol",1309607671),new cljs.core.Keyword(null,"spawn-predicate","spawn-predicate",543545894),new cljs.core.Keyword(null,"image","image",-58725096));
if(cljs.core.record_QMARK_.call(null,G__39680)){
return cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,G__39689);
} else {
return G__39689;
}
})();
return (new four_nations.model.map.resources.Resource(new cljs.core.Keyword(null,"name","name",1843675177).cljs$core$IFn$_invoke$arity$1(G__39680),new cljs.core.Keyword(null,"display-name","display-name",694513143).cljs$core$IFn$_invoke$arity$1(G__39680),new cljs.core.Keyword(null,"text-symbol","text-symbol",1309607671).cljs$core$IFn$_invoke$arity$1(G__39680),new cljs.core.Keyword(null,"spawn-predicate","spawn-predicate",543545894).cljs$core$IFn$_invoke$arity$1(G__39680),new cljs.core.Keyword(null,"image","image",-58725096).cljs$core$IFn$_invoke$arity$1(G__39680),null,cljs.core.not_empty.call(null,extmap__4419__auto__),null));
});

/**
 * Ensures that the resource has all of the basic required fields.
 */
four_nations.model.map.resources.valid_resource_definition_QMARK_ = (function four_nations$model$map$resources$valid_resource_definition_QMARK_(resource){
var and__4115__auto__ = new cljs.core.Keyword(null,"name","name",1843675177).cljs$core$IFn$_invoke$arity$1(resource);
if(cljs.core.truth_(and__4115__auto__)){
var and__4115__auto____$1 = new cljs.core.Keyword(null,"symbol","symbol",-1038572696).cljs$core$IFn$_invoke$arity$1(resource);
if(cljs.core.truth_(and__4115__auto____$1)){
return new cljs.core.Keyword(null,"spawning","spawning",-243970190).cljs$core$IFn$_invoke$arity$1(resource);
} else {
return and__4115__auto____$1;
}
} else {
return and__4115__auto__;
}
});
four_nations.model.map.resources.spawn_resource_on_tile_QMARK_ = (function four_nations$model$map$resources$spawn_resource_on_tile_QMARK_(resource,tile){
var spawn_chance = cljs.core.get_in.call(null,resource,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"spawning","spawning",-243970190),new cljs.core.Keyword(null,"chance","chance",-1877227883)], null));
var allowed_biomes = cljs.core.get_in.call(null,resource,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"spawning","spawning",-243970190),new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"biomes","biomes",1444240594)], null));
var allowed_terrain_types = cljs.core.get_in.call(null,resource,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"spawning","spawning",-243970190),new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"terrain-types","terrain-types",507721131)], null));
var tile_biome = cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"biome","biome",1856013988),new cljs.core.Keyword(null,"name","name",1843675177)], null));
var tile_terrain_type = cljs.core.get_in.call(null,tile,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"attributes","attributes",-74013604),new cljs.core.Keyword(null,"terrain-type","terrain-type",-1982390111)], null));
return (((cljs.core.rand.call(null) < spawn_chance)) && ((((allowed_biomes == null)) || (cljs.core.contains_QMARK_.call(null,allowed_biomes,tile_biome)))) && ((((allowed_terrain_types == null)) || (cljs.core.contains_QMARK_.call(null,allowed_terrain_types,tile_terrain_type)))));
});
/**
 * Given an EDN resource definiton, validations that definition and builds the Resource record. This
 * resource record will include the tile-based spawn predicate.
 */
four_nations.model.map.resources.resource_definition__GT_resource = (function four_nations$model$map$resources$resource_definition__GT_resource(resource_definition){
if(cljs.core.truth_(four_nations.model.map.resources.valid_resource_definition_QMARK_.call(null,resource_definition))){
var spawn_predicate = cljs.core.partial.call(null,four_nations.model.map.resources.spawn_resource_on_tile_QMARK_,resource_definition);
return four_nations.model.map.resources.map__GT_Resource.call(null,cljs.core.assoc.call(null,resource_definition,new cljs.core.Keyword(null,"spawn-predicate","spawn-predicate",543545894),spawn_predicate));
} else {
throw cljs.core.ex_info.call(null,"Invalid resource definition!",resource_definition);
}
});

//# sourceMappingURL=resources.js.map?rel=1654487547662
