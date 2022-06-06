// Compiled by ClojureScript 1.10.773 {}
goog.provide('four_nations.general.types');
goog.require('cljs.core');

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
four_nations.general.types.Dimension = (function (height,width,__meta,__extmap,__hash){
this.height = height;
this.width = width;
this.__meta = __meta;
this.__extmap = __extmap;
this.__hash = __hash;
this.cljs$lang$protocol_mask$partition0$ = 2230716170;
this.cljs$lang$protocol_mask$partition1$ = 139264;
});
(four_nations.general.types.Dimension.prototype.cljs$core$ILookup$_lookup$arity$2 = (function (this__4380__auto__,k__4381__auto__){
var self__ = this;
var this__4380__auto____$1 = this;
return this__4380__auto____$1.cljs$core$ILookup$_lookup$arity$3(null,k__4381__auto__,null);
}));

(four_nations.general.types.Dimension.prototype.cljs$core$ILookup$_lookup$arity$3 = (function (this__4382__auto__,k39219,else__4383__auto__){
var self__ = this;
var this__4382__auto____$1 = this;
var G__39223 = k39219;
var G__39223__$1 = (((G__39223 instanceof cljs.core.Keyword))?G__39223.fqn:null);
switch (G__39223__$1) {
case "height":
return self__.height;

break;
case "width":
return self__.width;

break;
default:
return cljs.core.get.call(null,self__.__extmap,k39219,else__4383__auto__);

}
}));

(four_nations.general.types.Dimension.prototype.cljs$core$IKVReduce$_kv_reduce$arity$3 = (function (this__4399__auto__,f__4400__auto__,init__4401__auto__){
var self__ = this;
var this__4399__auto____$1 = this;
return cljs.core.reduce.call(null,(function (ret__4402__auto__,p__39224){
var vec__39225 = p__39224;
var k__4403__auto__ = cljs.core.nth.call(null,vec__39225,(0),null);
var v__4404__auto__ = cljs.core.nth.call(null,vec__39225,(1),null);
return f__4400__auto__.call(null,ret__4402__auto__,k__4403__auto__,v__4404__auto__);
}),init__4401__auto__,this__4399__auto____$1);
}));

(four_nations.general.types.Dimension.prototype.cljs$core$IPrintWithWriter$_pr_writer$arity$3 = (function (this__4394__auto__,writer__4395__auto__,opts__4396__auto__){
var self__ = this;
var this__4394__auto____$1 = this;
var pr_pair__4397__auto__ = (function (keyval__4398__auto__){
return cljs.core.pr_sequential_writer.call(null,writer__4395__auto__,cljs.core.pr_writer,""," ","",opts__4396__auto__,keyval__4398__auto__);
});
return cljs.core.pr_sequential_writer.call(null,writer__4395__auto__,pr_pair__4397__auto__,"#four-nations.general.types.Dimension{",", ","}",opts__4396__auto__,cljs.core.concat.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"height","height",1025178622),self__.height],null)),(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"width","width",-384071477),self__.width],null))], null),self__.__extmap));
}));

(four_nations.general.types.Dimension.prototype.cljs$core$IIterable$_iterator$arity$1 = (function (G__39218){
var self__ = this;
var G__39218__$1 = this;
return (new cljs.core.RecordIter((0),G__39218__$1,2,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"height","height",1025178622),new cljs.core.Keyword(null,"width","width",-384071477)], null),(cljs.core.truth_(self__.__extmap)?cljs.core._iterator.call(null,self__.__extmap):cljs.core.nil_iter.call(null))));
}));

(four_nations.general.types.Dimension.prototype.cljs$core$IMeta$_meta$arity$1 = (function (this__4378__auto__){
var self__ = this;
var this__4378__auto____$1 = this;
return self__.__meta;
}));

(four_nations.general.types.Dimension.prototype.cljs$core$ICloneable$_clone$arity$1 = (function (this__4375__auto__){
var self__ = this;
var this__4375__auto____$1 = this;
return (new four_nations.general.types.Dimension(self__.height,self__.width,self__.__meta,self__.__extmap,self__.__hash));
}));

(four_nations.general.types.Dimension.prototype.cljs$core$ICounted$_count$arity$1 = (function (this__4384__auto__){
var self__ = this;
var this__4384__auto____$1 = this;
return (2 + cljs.core.count.call(null,self__.__extmap));
}));

(four_nations.general.types.Dimension.prototype.cljs$core$IHash$_hash$arity$1 = (function (this__4376__auto__){
var self__ = this;
var this__4376__auto____$1 = this;
var h__4238__auto__ = self__.__hash;
if((!((h__4238__auto__ == null)))){
return h__4238__auto__;
} else {
var h__4238__auto____$1 = (function (coll__4377__auto__){
return (-1862877445 ^ cljs.core.hash_unordered_coll.call(null,coll__4377__auto__));
}).call(null,this__4376__auto____$1);
(self__.__hash = h__4238__auto____$1);

return h__4238__auto____$1;
}
}));

(four_nations.general.types.Dimension.prototype.cljs$core$IEquiv$_equiv$arity$2 = (function (this39220,other39221){
var self__ = this;
var this39220__$1 = this;
return (((!((other39221 == null)))) && ((this39220__$1.constructor === other39221.constructor)) && (cljs.core._EQ_.call(null,this39220__$1.height,other39221.height)) && (cljs.core._EQ_.call(null,this39220__$1.width,other39221.width)) && (cljs.core._EQ_.call(null,this39220__$1.__extmap,other39221.__extmap)));
}));

(four_nations.general.types.Dimension.prototype.cljs$core$IMap$_dissoc$arity$2 = (function (this__4389__auto__,k__4390__auto__){
var self__ = this;
var this__4389__auto____$1 = this;
if(cljs.core.contains_QMARK_.call(null,new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"width","width",-384071477),null,new cljs.core.Keyword(null,"height","height",1025178622),null], null), null),k__4390__auto__)){
return cljs.core.dissoc.call(null,cljs.core._with_meta.call(null,cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,this__4389__auto____$1),self__.__meta),k__4390__auto__);
} else {
return (new four_nations.general.types.Dimension(self__.height,self__.width,self__.__meta,cljs.core.not_empty.call(null,cljs.core.dissoc.call(null,self__.__extmap,k__4390__auto__)),null));
}
}));

(four_nations.general.types.Dimension.prototype.cljs$core$IAssociative$_assoc$arity$3 = (function (this__4387__auto__,k__4388__auto__,G__39218){
var self__ = this;
var this__4387__auto____$1 = this;
var pred__39228 = cljs.core.keyword_identical_QMARK_;
var expr__39229 = k__4388__auto__;
if(cljs.core.truth_(pred__39228.call(null,new cljs.core.Keyword(null,"height","height",1025178622),expr__39229))){
return (new four_nations.general.types.Dimension(G__39218,self__.width,self__.__meta,self__.__extmap,null));
} else {
if(cljs.core.truth_(pred__39228.call(null,new cljs.core.Keyword(null,"width","width",-384071477),expr__39229))){
return (new four_nations.general.types.Dimension(self__.height,G__39218,self__.__meta,self__.__extmap,null));
} else {
return (new four_nations.general.types.Dimension(self__.height,self__.width,self__.__meta,cljs.core.assoc.call(null,self__.__extmap,k__4388__auto__,G__39218),null));
}
}
}));

(four_nations.general.types.Dimension.prototype.cljs$core$ISeqable$_seq$arity$1 = (function (this__4392__auto__){
var self__ = this;
var this__4392__auto____$1 = this;
return cljs.core.seq.call(null,cljs.core.concat.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [(new cljs.core.MapEntry(new cljs.core.Keyword(null,"height","height",1025178622),self__.height,null)),(new cljs.core.MapEntry(new cljs.core.Keyword(null,"width","width",-384071477),self__.width,null))], null),self__.__extmap));
}));

(four_nations.general.types.Dimension.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (this__4379__auto__,G__39218){
var self__ = this;
var this__4379__auto____$1 = this;
return (new four_nations.general.types.Dimension(self__.height,self__.width,G__39218,self__.__extmap,self__.__hash));
}));

(four_nations.general.types.Dimension.prototype.cljs$core$ICollection$_conj$arity$2 = (function (this__4385__auto__,entry__4386__auto__){
var self__ = this;
var this__4385__auto____$1 = this;
if(cljs.core.vector_QMARK_.call(null,entry__4386__auto__)){
return this__4385__auto____$1.cljs$core$IAssociative$_assoc$arity$3(null,cljs.core._nth.call(null,entry__4386__auto__,(0)),cljs.core._nth.call(null,entry__4386__auto__,(1)));
} else {
return cljs.core.reduce.call(null,cljs.core._conj,this__4385__auto____$1,entry__4386__auto__);
}
}));

(four_nations.general.types.Dimension.getBasis = (function (){
return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"height","height",-1629257147,null),new cljs.core.Symbol(null,"width","width",1256460050,null)], null);
}));

(four_nations.general.types.Dimension.cljs$lang$type = true);

(four_nations.general.types.Dimension.cljs$lang$ctorPrSeq = (function (this__4423__auto__){
return (new cljs.core.List(null,"four-nations.general.types/Dimension",null,(1),null));
}));

(four_nations.general.types.Dimension.cljs$lang$ctorPrWriter = (function (this__4423__auto__,writer__4424__auto__){
return cljs.core._write.call(null,writer__4424__auto__,"four-nations.general.types/Dimension");
}));

/**
 * Positional factory function for four-nations.general.types/Dimension.
 */
four_nations.general.types.__GT_Dimension = (function four_nations$general$types$__GT_Dimension(height,width){
return (new four_nations.general.types.Dimension(height,width,null,null,null));
});

/**
 * Factory function for four-nations.general.types/Dimension, taking a map of keywords to field values.
 */
four_nations.general.types.map__GT_Dimension = (function four_nations$general$types$map__GT_Dimension(G__39222){
var extmap__4419__auto__ = (function (){var G__39231 = cljs.core.dissoc.call(null,G__39222,new cljs.core.Keyword(null,"height","height",1025178622),new cljs.core.Keyword(null,"width","width",-384071477));
if(cljs.core.record_QMARK_.call(null,G__39222)){
return cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,G__39231);
} else {
return G__39231;
}
})();
return (new four_nations.general.types.Dimension(new cljs.core.Keyword(null,"height","height",1025178622).cljs$core$IFn$_invoke$arity$1(G__39222),new cljs.core.Keyword(null,"width","width",-384071477).cljs$core$IFn$_invoke$arity$1(G__39222),null,cljs.core.not_empty.call(null,extmap__4419__auto__),null));
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
four_nations.general.types.Point = (function (x,y,__meta,__extmap,__hash){
this.x = x;
this.y = y;
this.__meta = __meta;
this.__extmap = __extmap;
this.__hash = __hash;
this.cljs$lang$protocol_mask$partition0$ = 2230716170;
this.cljs$lang$protocol_mask$partition1$ = 139264;
});
(four_nations.general.types.Point.prototype.cljs$core$ILookup$_lookup$arity$2 = (function (this__4380__auto__,k__4381__auto__){
var self__ = this;
var this__4380__auto____$1 = this;
return this__4380__auto____$1.cljs$core$ILookup$_lookup$arity$3(null,k__4381__auto__,null);
}));

(four_nations.general.types.Point.prototype.cljs$core$ILookup$_lookup$arity$3 = (function (this__4382__auto__,k39234,else__4383__auto__){
var self__ = this;
var this__4382__auto____$1 = this;
var G__39238 = k39234;
var G__39238__$1 = (((G__39238 instanceof cljs.core.Keyword))?G__39238.fqn:null);
switch (G__39238__$1) {
case "x":
return self__.x;

break;
case "y":
return self__.y;

break;
default:
return cljs.core.get.call(null,self__.__extmap,k39234,else__4383__auto__);

}
}));

(four_nations.general.types.Point.prototype.cljs$core$IKVReduce$_kv_reduce$arity$3 = (function (this__4399__auto__,f__4400__auto__,init__4401__auto__){
var self__ = this;
var this__4399__auto____$1 = this;
return cljs.core.reduce.call(null,(function (ret__4402__auto__,p__39239){
var vec__39240 = p__39239;
var k__4403__auto__ = cljs.core.nth.call(null,vec__39240,(0),null);
var v__4404__auto__ = cljs.core.nth.call(null,vec__39240,(1),null);
return f__4400__auto__.call(null,ret__4402__auto__,k__4403__auto__,v__4404__auto__);
}),init__4401__auto__,this__4399__auto____$1);
}));

(four_nations.general.types.Point.prototype.cljs$core$IPrintWithWriter$_pr_writer$arity$3 = (function (this__4394__auto__,writer__4395__auto__,opts__4396__auto__){
var self__ = this;
var this__4394__auto____$1 = this;
var pr_pair__4397__auto__ = (function (keyval__4398__auto__){
return cljs.core.pr_sequential_writer.call(null,writer__4395__auto__,cljs.core.pr_writer,""," ","",opts__4396__auto__,keyval__4398__auto__);
});
return cljs.core.pr_sequential_writer.call(null,writer__4395__auto__,pr_pair__4397__auto__,"#four-nations.general.types.Point{",", ","}",opts__4396__auto__,cljs.core.concat.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"x","x",2099068185),self__.x],null)),(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"y","y",-1757859776),self__.y],null))], null),self__.__extmap));
}));

(four_nations.general.types.Point.prototype.cljs$core$IIterable$_iterator$arity$1 = (function (G__39233){
var self__ = this;
var G__39233__$1 = this;
return (new cljs.core.RecordIter((0),G__39233__$1,2,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"x","x",2099068185),new cljs.core.Keyword(null,"y","y",-1757859776)], null),(cljs.core.truth_(self__.__extmap)?cljs.core._iterator.call(null,self__.__extmap):cljs.core.nil_iter.call(null))));
}));

(four_nations.general.types.Point.prototype.cljs$core$IMeta$_meta$arity$1 = (function (this__4378__auto__){
var self__ = this;
var this__4378__auto____$1 = this;
return self__.__meta;
}));

(four_nations.general.types.Point.prototype.cljs$core$ICloneable$_clone$arity$1 = (function (this__4375__auto__){
var self__ = this;
var this__4375__auto____$1 = this;
return (new four_nations.general.types.Point(self__.x,self__.y,self__.__meta,self__.__extmap,self__.__hash));
}));

(four_nations.general.types.Point.prototype.cljs$core$ICounted$_count$arity$1 = (function (this__4384__auto__){
var self__ = this;
var this__4384__auto____$1 = this;
return (2 + cljs.core.count.call(null,self__.__extmap));
}));

(four_nations.general.types.Point.prototype.cljs$core$IHash$_hash$arity$1 = (function (this__4376__auto__){
var self__ = this;
var this__4376__auto____$1 = this;
var h__4238__auto__ = self__.__hash;
if((!((h__4238__auto__ == null)))){
return h__4238__auto__;
} else {
var h__4238__auto____$1 = (function (coll__4377__auto__){
return (-330848158 ^ cljs.core.hash_unordered_coll.call(null,coll__4377__auto__));
}).call(null,this__4376__auto____$1);
(self__.__hash = h__4238__auto____$1);

return h__4238__auto____$1;
}
}));

(four_nations.general.types.Point.prototype.cljs$core$IEquiv$_equiv$arity$2 = (function (this39235,other39236){
var self__ = this;
var this39235__$1 = this;
return (((!((other39236 == null)))) && ((this39235__$1.constructor === other39236.constructor)) && (cljs.core._EQ_.call(null,this39235__$1.x,other39236.x)) && (cljs.core._EQ_.call(null,this39235__$1.y,other39236.y)) && (cljs.core._EQ_.call(null,this39235__$1.__extmap,other39236.__extmap)));
}));

(four_nations.general.types.Point.prototype.cljs$core$IMap$_dissoc$arity$2 = (function (this__4389__auto__,k__4390__auto__){
var self__ = this;
var this__4389__auto____$1 = this;
if(cljs.core.contains_QMARK_.call(null,new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"y","y",-1757859776),null,new cljs.core.Keyword(null,"x","x",2099068185),null], null), null),k__4390__auto__)){
return cljs.core.dissoc.call(null,cljs.core._with_meta.call(null,cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,this__4389__auto____$1),self__.__meta),k__4390__auto__);
} else {
return (new four_nations.general.types.Point(self__.x,self__.y,self__.__meta,cljs.core.not_empty.call(null,cljs.core.dissoc.call(null,self__.__extmap,k__4390__auto__)),null));
}
}));

(four_nations.general.types.Point.prototype.cljs$core$IAssociative$_assoc$arity$3 = (function (this__4387__auto__,k__4388__auto__,G__39233){
var self__ = this;
var this__4387__auto____$1 = this;
var pred__39243 = cljs.core.keyword_identical_QMARK_;
var expr__39244 = k__4388__auto__;
if(cljs.core.truth_(pred__39243.call(null,new cljs.core.Keyword(null,"x","x",2099068185),expr__39244))){
return (new four_nations.general.types.Point(G__39233,self__.y,self__.__meta,self__.__extmap,null));
} else {
if(cljs.core.truth_(pred__39243.call(null,new cljs.core.Keyword(null,"y","y",-1757859776),expr__39244))){
return (new four_nations.general.types.Point(self__.x,G__39233,self__.__meta,self__.__extmap,null));
} else {
return (new four_nations.general.types.Point(self__.x,self__.y,self__.__meta,cljs.core.assoc.call(null,self__.__extmap,k__4388__auto__,G__39233),null));
}
}
}));

(four_nations.general.types.Point.prototype.cljs$core$ISeqable$_seq$arity$1 = (function (this__4392__auto__){
var self__ = this;
var this__4392__auto____$1 = this;
return cljs.core.seq.call(null,cljs.core.concat.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [(new cljs.core.MapEntry(new cljs.core.Keyword(null,"x","x",2099068185),self__.x,null)),(new cljs.core.MapEntry(new cljs.core.Keyword(null,"y","y",-1757859776),self__.y,null))], null),self__.__extmap));
}));

(four_nations.general.types.Point.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (this__4379__auto__,G__39233){
var self__ = this;
var this__4379__auto____$1 = this;
return (new four_nations.general.types.Point(self__.x,self__.y,G__39233,self__.__extmap,self__.__hash));
}));

(four_nations.general.types.Point.prototype.cljs$core$ICollection$_conj$arity$2 = (function (this__4385__auto__,entry__4386__auto__){
var self__ = this;
var this__4385__auto____$1 = this;
if(cljs.core.vector_QMARK_.call(null,entry__4386__auto__)){
return this__4385__auto____$1.cljs$core$IAssociative$_assoc$arity$3(null,cljs.core._nth.call(null,entry__4386__auto__,(0)),cljs.core._nth.call(null,entry__4386__auto__,(1)));
} else {
return cljs.core.reduce.call(null,cljs.core._conj,this__4385__auto____$1,entry__4386__auto__);
}
}));

(four_nations.general.types.Point.getBasis = (function (){
return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"x","x",-555367584,null),new cljs.core.Symbol(null,"y","y",-117328249,null)], null);
}));

(four_nations.general.types.Point.cljs$lang$type = true);

(four_nations.general.types.Point.cljs$lang$ctorPrSeq = (function (this__4423__auto__){
return (new cljs.core.List(null,"four-nations.general.types/Point",null,(1),null));
}));

(four_nations.general.types.Point.cljs$lang$ctorPrWriter = (function (this__4423__auto__,writer__4424__auto__){
return cljs.core._write.call(null,writer__4424__auto__,"four-nations.general.types/Point");
}));

/**
 * Positional factory function for four-nations.general.types/Point.
 */
four_nations.general.types.__GT_Point = (function four_nations$general$types$__GT_Point(x,y){
return (new four_nations.general.types.Point(x,y,null,null,null));
});

/**
 * Factory function for four-nations.general.types/Point, taking a map of keywords to field values.
 */
four_nations.general.types.map__GT_Point = (function four_nations$general$types$map__GT_Point(G__39237){
var extmap__4419__auto__ = (function (){var G__39246 = cljs.core.dissoc.call(null,G__39237,new cljs.core.Keyword(null,"x","x",2099068185),new cljs.core.Keyword(null,"y","y",-1757859776));
if(cljs.core.record_QMARK_.call(null,G__39237)){
return cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,G__39246);
} else {
return G__39246;
}
})();
return (new four_nations.general.types.Point(new cljs.core.Keyword(null,"x","x",2099068185).cljs$core$IFn$_invoke$arity$1(G__39237),new cljs.core.Keyword(null,"y","y",-1757859776).cljs$core$IFn$_invoke$arity$1(G__39237),null,cljs.core.not_empty.call(null,extmap__4419__auto__),null));
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
four_nations.general.types.MapTile = (function (point,value,attributes,__meta,__extmap,__hash){
this.point = point;
this.value = value;
this.attributes = attributes;
this.__meta = __meta;
this.__extmap = __extmap;
this.__hash = __hash;
this.cljs$lang$protocol_mask$partition0$ = 2230716170;
this.cljs$lang$protocol_mask$partition1$ = 139264;
});
(four_nations.general.types.MapTile.prototype.cljs$core$ILookup$_lookup$arity$2 = (function (this__4380__auto__,k__4381__auto__){
var self__ = this;
var this__4380__auto____$1 = this;
return this__4380__auto____$1.cljs$core$ILookup$_lookup$arity$3(null,k__4381__auto__,null);
}));

(four_nations.general.types.MapTile.prototype.cljs$core$ILookup$_lookup$arity$3 = (function (this__4382__auto__,k39249,else__4383__auto__){
var self__ = this;
var this__4382__auto____$1 = this;
var G__39253 = k39249;
var G__39253__$1 = (((G__39253 instanceof cljs.core.Keyword))?G__39253.fqn:null);
switch (G__39253__$1) {
case "point":
return self__.point;

break;
case "value":
return self__.value;

break;
case "attributes":
return self__.attributes;

break;
default:
return cljs.core.get.call(null,self__.__extmap,k39249,else__4383__auto__);

}
}));

(four_nations.general.types.MapTile.prototype.cljs$core$IKVReduce$_kv_reduce$arity$3 = (function (this__4399__auto__,f__4400__auto__,init__4401__auto__){
var self__ = this;
var this__4399__auto____$1 = this;
return cljs.core.reduce.call(null,(function (ret__4402__auto__,p__39254){
var vec__39255 = p__39254;
var k__4403__auto__ = cljs.core.nth.call(null,vec__39255,(0),null);
var v__4404__auto__ = cljs.core.nth.call(null,vec__39255,(1),null);
return f__4400__auto__.call(null,ret__4402__auto__,k__4403__auto__,v__4404__auto__);
}),init__4401__auto__,this__4399__auto____$1);
}));

(four_nations.general.types.MapTile.prototype.cljs$core$IPrintWithWriter$_pr_writer$arity$3 = (function (this__4394__auto__,writer__4395__auto__,opts__4396__auto__){
var self__ = this;
var this__4394__auto____$1 = this;
var pr_pair__4397__auto__ = (function (keyval__4398__auto__){
return cljs.core.pr_sequential_writer.call(null,writer__4395__auto__,cljs.core.pr_writer,""," ","",opts__4396__auto__,keyval__4398__auto__);
});
return cljs.core.pr_sequential_writer.call(null,writer__4395__auto__,pr_pair__4397__auto__,"#four-nations.general.types.MapTile{",", ","}",opts__4396__auto__,cljs.core.concat.call(null,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"point","point",1813198264),self__.point],null)),(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"value","value",305978217),self__.value],null)),(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[new cljs.core.Keyword(null,"attributes","attributes",-74013604),self__.attributes],null))], null),self__.__extmap));
}));

(four_nations.general.types.MapTile.prototype.cljs$core$IIterable$_iterator$arity$1 = (function (G__39248){
var self__ = this;
var G__39248__$1 = this;
return (new cljs.core.RecordIter((0),G__39248__$1,3,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"point","point",1813198264),new cljs.core.Keyword(null,"value","value",305978217),new cljs.core.Keyword(null,"attributes","attributes",-74013604)], null),(cljs.core.truth_(self__.__extmap)?cljs.core._iterator.call(null,self__.__extmap):cljs.core.nil_iter.call(null))));
}));

(four_nations.general.types.MapTile.prototype.cljs$core$IMeta$_meta$arity$1 = (function (this__4378__auto__){
var self__ = this;
var this__4378__auto____$1 = this;
return self__.__meta;
}));

(four_nations.general.types.MapTile.prototype.cljs$core$ICloneable$_clone$arity$1 = (function (this__4375__auto__){
var self__ = this;
var this__4375__auto____$1 = this;
return (new four_nations.general.types.MapTile(self__.point,self__.value,self__.attributes,self__.__meta,self__.__extmap,self__.__hash));
}));

(four_nations.general.types.MapTile.prototype.cljs$core$ICounted$_count$arity$1 = (function (this__4384__auto__){
var self__ = this;
var this__4384__auto____$1 = this;
return (3 + cljs.core.count.call(null,self__.__extmap));
}));

(four_nations.general.types.MapTile.prototype.cljs$core$IHash$_hash$arity$1 = (function (this__4376__auto__){
var self__ = this;
var this__4376__auto____$1 = this;
var h__4238__auto__ = self__.__hash;
if((!((h__4238__auto__ == null)))){
return h__4238__auto__;
} else {
var h__4238__auto____$1 = (function (coll__4377__auto__){
return (-1879752762 ^ cljs.core.hash_unordered_coll.call(null,coll__4377__auto__));
}).call(null,this__4376__auto____$1);
(self__.__hash = h__4238__auto____$1);

return h__4238__auto____$1;
}
}));

(four_nations.general.types.MapTile.prototype.cljs$core$IEquiv$_equiv$arity$2 = (function (this39250,other39251){
var self__ = this;
var this39250__$1 = this;
return (((!((other39251 == null)))) && ((this39250__$1.constructor === other39251.constructor)) && (cljs.core._EQ_.call(null,this39250__$1.point,other39251.point)) && (cljs.core._EQ_.call(null,this39250__$1.value,other39251.value)) && (cljs.core._EQ_.call(null,this39250__$1.attributes,other39251.attributes)) && (cljs.core._EQ_.call(null,this39250__$1.__extmap,other39251.__extmap)));
}));

(four_nations.general.types.MapTile.prototype.cljs$core$IMap$_dissoc$arity$2 = (function (this__4389__auto__,k__4390__auto__){
var self__ = this;
var this__4389__auto____$1 = this;
if(cljs.core.contains_QMARK_.call(null,new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"value","value",305978217),null,new cljs.core.Keyword(null,"point","point",1813198264),null,new cljs.core.Keyword(null,"attributes","attributes",-74013604),null], null), null),k__4390__auto__)){
return cljs.core.dissoc.call(null,cljs.core._with_meta.call(null,cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,this__4389__auto____$1),self__.__meta),k__4390__auto__);
} else {
return (new four_nations.general.types.MapTile(self__.point,self__.value,self__.attributes,self__.__meta,cljs.core.not_empty.call(null,cljs.core.dissoc.call(null,self__.__extmap,k__4390__auto__)),null));
}
}));

(four_nations.general.types.MapTile.prototype.cljs$core$IAssociative$_assoc$arity$3 = (function (this__4387__auto__,k__4388__auto__,G__39248){
var self__ = this;
var this__4387__auto____$1 = this;
var pred__39258 = cljs.core.keyword_identical_QMARK_;
var expr__39259 = k__4388__auto__;
if(cljs.core.truth_(pred__39258.call(null,new cljs.core.Keyword(null,"point","point",1813198264),expr__39259))){
return (new four_nations.general.types.MapTile(G__39248,self__.value,self__.attributes,self__.__meta,self__.__extmap,null));
} else {
if(cljs.core.truth_(pred__39258.call(null,new cljs.core.Keyword(null,"value","value",305978217),expr__39259))){
return (new four_nations.general.types.MapTile(self__.point,G__39248,self__.attributes,self__.__meta,self__.__extmap,null));
} else {
if(cljs.core.truth_(pred__39258.call(null,new cljs.core.Keyword(null,"attributes","attributes",-74013604),expr__39259))){
return (new four_nations.general.types.MapTile(self__.point,self__.value,G__39248,self__.__meta,self__.__extmap,null));
} else {
return (new four_nations.general.types.MapTile(self__.point,self__.value,self__.attributes,self__.__meta,cljs.core.assoc.call(null,self__.__extmap,k__4388__auto__,G__39248),null));
}
}
}
}));

(four_nations.general.types.MapTile.prototype.cljs$core$ISeqable$_seq$arity$1 = (function (this__4392__auto__){
var self__ = this;
var this__4392__auto____$1 = this;
return cljs.core.seq.call(null,cljs.core.concat.call(null,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [(new cljs.core.MapEntry(new cljs.core.Keyword(null,"point","point",1813198264),self__.point,null)),(new cljs.core.MapEntry(new cljs.core.Keyword(null,"value","value",305978217),self__.value,null)),(new cljs.core.MapEntry(new cljs.core.Keyword(null,"attributes","attributes",-74013604),self__.attributes,null))], null),self__.__extmap));
}));

(four_nations.general.types.MapTile.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (this__4379__auto__,G__39248){
var self__ = this;
var this__4379__auto____$1 = this;
return (new four_nations.general.types.MapTile(self__.point,self__.value,self__.attributes,G__39248,self__.__extmap,self__.__hash));
}));

(four_nations.general.types.MapTile.prototype.cljs$core$ICollection$_conj$arity$2 = (function (this__4385__auto__,entry__4386__auto__){
var self__ = this;
var this__4385__auto____$1 = this;
if(cljs.core.vector_QMARK_.call(null,entry__4386__auto__)){
return this__4385__auto____$1.cljs$core$IAssociative$_assoc$arity$3(null,cljs.core._nth.call(null,entry__4386__auto__,(0)),cljs.core._nth.call(null,entry__4386__auto__,(1)));
} else {
return cljs.core.reduce.call(null,cljs.core._conj,this__4385__auto____$1,entry__4386__auto__);
}
}));

(four_nations.general.types.MapTile.getBasis = (function (){
return new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"point","point",-841237505,null),new cljs.core.Symbol(null,"value","value",1946509744,null),new cljs.core.Symbol(null,"attributes","attributes",1566517923,null)], null);
}));

(four_nations.general.types.MapTile.cljs$lang$type = true);

(four_nations.general.types.MapTile.cljs$lang$ctorPrSeq = (function (this__4423__auto__){
return (new cljs.core.List(null,"four-nations.general.types/MapTile",null,(1),null));
}));

(four_nations.general.types.MapTile.cljs$lang$ctorPrWriter = (function (this__4423__auto__,writer__4424__auto__){
return cljs.core._write.call(null,writer__4424__auto__,"four-nations.general.types/MapTile");
}));

/**
 * Positional factory function for four-nations.general.types/MapTile.
 */
four_nations.general.types.__GT_MapTile = (function four_nations$general$types$__GT_MapTile(point,value,attributes){
return (new four_nations.general.types.MapTile(point,value,attributes,null,null,null));
});

/**
 * Factory function for four-nations.general.types/MapTile, taking a map of keywords to field values.
 */
four_nations.general.types.map__GT_MapTile = (function four_nations$general$types$map__GT_MapTile(G__39252){
var extmap__4419__auto__ = (function (){var G__39261 = cljs.core.dissoc.call(null,G__39252,new cljs.core.Keyword(null,"point","point",1813198264),new cljs.core.Keyword(null,"value","value",305978217),new cljs.core.Keyword(null,"attributes","attributes",-74013604));
if(cljs.core.record_QMARK_.call(null,G__39252)){
return cljs.core.into.call(null,cljs.core.PersistentArrayMap.EMPTY,G__39261);
} else {
return G__39261;
}
})();
return (new four_nations.general.types.MapTile(new cljs.core.Keyword(null,"point","point",1813198264).cljs$core$IFn$_invoke$arity$1(G__39252),new cljs.core.Keyword(null,"value","value",305978217).cljs$core$IFn$_invoke$arity$1(G__39252),new cljs.core.Keyword(null,"attributes","attributes",-74013604).cljs$core$IFn$_invoke$arity$1(G__39252),null,cljs.core.not_empty.call(null,extmap__4419__auto__),null));
});


//# sourceMappingURL=types.js.map?rel=1654487546939
