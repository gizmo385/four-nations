// Compiled by ClojureScript 1.10.773 {}
goog.provide('figwheel.client.file_reloading');
goog.require('cljs.core');
goog.require('figwheel.client.utils');
goog.require('goog.Uri');
goog.require('goog.object');
goog.require('goog.net.jsloader');
goog.require('goog.html.legacyconversions');
goog.require('goog.string');
goog.require('clojure.string');
goog.require('clojure.set');
goog.require('cljs.core.async');
goog.require('goog.async.Deferred');
if((typeof figwheel !== 'undefined') && (typeof figwheel.client !== 'undefined') && (typeof figwheel.client.file_reloading !== 'undefined') && (typeof figwheel.client.file_reloading.figwheel_meta_pragmas !== 'undefined')){
} else {
figwheel.client.file_reloading.figwheel_meta_pragmas = cljs.core.atom.call(null,cljs.core.PersistentArrayMap.EMPTY);
}
figwheel.client.file_reloading.debug_loader_QMARK_ = (function figwheel$client$file_reloading$debug_loader_QMARK_(){
return (!((goog.debugLoader_ == null)));
});
figwheel.client.file_reloading.on_jsload_custom_event = (function figwheel$client$file_reloading$on_jsload_custom_event(url){
return figwheel.client.utils.dispatch_custom_event.call(null,"figwheel.js-reload",url);
});
figwheel.client.file_reloading.before_jsload_custom_event = (function figwheel$client$file_reloading$before_jsload_custom_event(files){
return figwheel.client.utils.dispatch_custom_event.call(null,"figwheel.before-js-reload",files);
});
figwheel.client.file_reloading.on_cssload_custom_event = (function figwheel$client$file_reloading$on_cssload_custom_event(files){
return figwheel.client.utils.dispatch_custom_event.call(null,"figwheel.css-reload",files);
});
figwheel.client.file_reloading.namespace_file_map_QMARK_ = (function figwheel$client$file_reloading$namespace_file_map_QMARK_(m){
var or__4126__auto__ = ((cljs.core.map_QMARK_.call(null,m)) && (typeof new cljs.core.Keyword(null,"namespace","namespace",-377510372).cljs$core$IFn$_invoke$arity$1(m) === 'string') && ((((new cljs.core.Keyword(null,"file","file",-1269645878).cljs$core$IFn$_invoke$arity$1(m) == null)) || (typeof new cljs.core.Keyword(null,"file","file",-1269645878).cljs$core$IFn$_invoke$arity$1(m) === 'string'))) && (cljs.core._EQ_.call(null,new cljs.core.Keyword(null,"type","type",1174270348).cljs$core$IFn$_invoke$arity$1(m),new cljs.core.Keyword(null,"namespace","namespace",-377510372))));
if(or__4126__auto__){
return or__4126__auto__;
} else {
cljs.core.println.call(null,"Error not namespace-file-map",cljs.core.pr_str.call(null,m));

return false;
}
});
figwheel.client.file_reloading.add_cache_buster = (function figwheel$client$file_reloading$add_cache_buster(url){

return goog.Uri.parse(url).makeUnique();
});
figwheel.client.file_reloading.name__GT_path = ((figwheel.client.file_reloading.debug_loader_QMARK_.call(null))?(function (ns){

return goog.debugLoader_.getPathFromDeps_(ns);
}):(function (ns){

return goog.object.get(goog.dependencies_.nameToPath,ns);
}));
figwheel.client.file_reloading.provided_QMARK_ = ((figwheel.client.file_reloading.debug_loader_QMARK_.call(null))?(function (ns){
return goog.getObjectByName(ns);
}):(function (ns){
return goog.object.get(goog.dependencies_.written,figwheel.client.file_reloading.name__GT_path.call(null,ns));
}));
figwheel.client.file_reloading.immutable_ns_QMARK_ = (function figwheel$client$file_reloading$immutable_ns_QMARK_(ns){
return ((cljs.core._EQ_.call(null,"goog",ns)) || (cljs.core._EQ_.call(null,"cljs.core",ns)) || (cljs.core._EQ_.call(null,"cljs.nodejs",ns)) || (goog.string.startsWith(ns,"clojure.")) || (goog.string.startsWith(ns,"goog.")));
});
figwheel.client.file_reloading.base_requires_for_ns_path = ((figwheel.client.file_reloading.debug_loader_QMARK_.call(null))?(function (path){
var G__44575 = goog.object.get(goog.debugLoader_.dependencies_,path);
if((G__44575 == null)){
return null;
} else {
return goog.object.get(G__44575,"requires");
}
}):(function (path){
var G__44576 = goog.object.get(goog.dependencies_.requires,path);
if((G__44576 == null)){
return null;
} else {
return goog.object.getKeys(G__44576);
}
}));
figwheel.client.file_reloading.get_requires = (function figwheel$client$file_reloading$get_requires(ns){
return cljs.core.set.call(null,cljs.core.filter.call(null,(function (p1__44577_SHARP_){
return (!(figwheel.client.file_reloading.immutable_ns_QMARK_.call(null,p1__44577_SHARP_)));
}),figwheel.client.file_reloading.base_requires_for_ns_path.call(null,figwheel.client.file_reloading.name__GT_path.call(null,ns))));
});
if((typeof figwheel !== 'undefined') && (typeof figwheel.client !== 'undefined') && (typeof figwheel.client.file_reloading !== 'undefined') && (typeof figwheel.client.file_reloading.dependency_data !== 'undefined')){
} else {
figwheel.client.file_reloading.dependency_data = cljs.core.atom.call(null,new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"pathToName","pathToName",-1236616181),cljs.core.PersistentArrayMap.EMPTY,new cljs.core.Keyword(null,"dependents","dependents",136812837),cljs.core.PersistentArrayMap.EMPTY], null));
}
figwheel.client.file_reloading.path_to_name_BANG_ = ((figwheel.client.file_reloading.debug_loader_QMARK_.call(null))?(function (_,___$1){
return null;
}):(function (path,name){
return cljs.core.swap_BANG_.call(null,figwheel.client.file_reloading.dependency_data,cljs.core.update_in,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"pathToName","pathToName",-1236616181),path], null),cljs.core.fnil.call(null,clojure.set.union,cljs.core.PersistentHashSet.EMPTY),cljs.core.PersistentHashSet.createAsIfByAssoc([name]));
}));
figwheel.client.file_reloading.setup_path__GT_name_BANG_ = ((figwheel.client.file_reloading.debug_loader_QMARK_.call(null))?(function (){
return null;
}):(function (){
var nameToPath = goog.object.filter(goog.dependencies_.nameToPath,(function (v,k,o){
return goog.string.startsWith(v,"../");
}));
return goog.object.forEach(nameToPath,(function (v,k,o){
return figwheel.client.file_reloading.path_to_name_BANG_.call(null,v,k);
}));
}));
figwheel.client.file_reloading.path__GT_name = ((figwheel.client.file_reloading.debug_loader_QMARK_.call(null))?(function (path){
var G__44578 = goog.object.get(goog.debugLoader_.dependencies_,path);
var G__44578__$1 = (((G__44578 == null))?null:goog.object.get(G__44578,"provides"));
if((G__44578__$1 == null)){
return null;
} else {
return cljs.core.set.call(null,G__44578__$1);
}
}):(function (path){
return cljs.core.get_in.call(null,cljs.core.deref.call(null,figwheel.client.file_reloading.dependency_data),new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"pathToName","pathToName",-1236616181),path], null));
}));
figwheel.client.file_reloading.name_to_parent_BANG_ = (function figwheel$client$file_reloading$name_to_parent_BANG_(ns,parent_ns){
return cljs.core.swap_BANG_.call(null,figwheel.client.file_reloading.dependency_data,cljs.core.update_in,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"dependents","dependents",136812837),ns], null),cljs.core.fnil.call(null,cljs.core.conj,cljs.core.PersistentHashSet.EMPTY),parent_ns);
});
figwheel.client.file_reloading.setup_ns__GT_dependents_BANG_ = ((figwheel.client.file_reloading.debug_loader_QMARK_.call(null))?(function (){
return goog.object.forEach(goog.object.filter(goog.debugLoader_.dependencies_,(function (dep,path,_){
return cljs.core.not.call(null,cljs.core.some.call(null,figwheel.client.file_reloading.immutable_ns_QMARK_,goog.object.get(dep,"provides")));
})),(function (dep,path,_){
var provides = goog.object.get(dep,"provides");
var requires = goog.object.get(dep,"requires");
var seq__44579 = cljs.core.seq.call(null,provides);
var chunk__44580 = null;
var count__44581 = (0);
var i__44582 = (0);
while(true){
if((i__44582 < count__44581)){
var prov = cljs.core._nth.call(null,chunk__44580,i__44582);
var seq__44591_44603 = cljs.core.seq.call(null,requires);
var chunk__44592_44604 = null;
var count__44593_44605 = (0);
var i__44594_44606 = (0);
while(true){
if((i__44594_44606 < count__44593_44605)){
var req_44607 = cljs.core._nth.call(null,chunk__44592_44604,i__44594_44606);
figwheel.client.file_reloading.name_to_parent_BANG_.call(null,req_44607,prov);


var G__44608 = seq__44591_44603;
var G__44609 = chunk__44592_44604;
var G__44610 = count__44593_44605;
var G__44611 = (i__44594_44606 + (1));
seq__44591_44603 = G__44608;
chunk__44592_44604 = G__44609;
count__44593_44605 = G__44610;
i__44594_44606 = G__44611;
continue;
} else {
var temp__5720__auto___44612 = cljs.core.seq.call(null,seq__44591_44603);
if(temp__5720__auto___44612){
var seq__44591_44613__$1 = temp__5720__auto___44612;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__44591_44613__$1)){
var c__4556__auto___44614 = cljs.core.chunk_first.call(null,seq__44591_44613__$1);
var G__44615 = cljs.core.chunk_rest.call(null,seq__44591_44613__$1);
var G__44616 = c__4556__auto___44614;
var G__44617 = cljs.core.count.call(null,c__4556__auto___44614);
var G__44618 = (0);
seq__44591_44603 = G__44615;
chunk__44592_44604 = G__44616;
count__44593_44605 = G__44617;
i__44594_44606 = G__44618;
continue;
} else {
var req_44619 = cljs.core.first.call(null,seq__44591_44613__$1);
figwheel.client.file_reloading.name_to_parent_BANG_.call(null,req_44619,prov);


var G__44620 = cljs.core.next.call(null,seq__44591_44613__$1);
var G__44621 = null;
var G__44622 = (0);
var G__44623 = (0);
seq__44591_44603 = G__44620;
chunk__44592_44604 = G__44621;
count__44593_44605 = G__44622;
i__44594_44606 = G__44623;
continue;
}
} else {
}
}
break;
}


var G__44624 = seq__44579;
var G__44625 = chunk__44580;
var G__44626 = count__44581;
var G__44627 = (i__44582 + (1));
seq__44579 = G__44624;
chunk__44580 = G__44625;
count__44581 = G__44626;
i__44582 = G__44627;
continue;
} else {
var temp__5720__auto__ = cljs.core.seq.call(null,seq__44579);
if(temp__5720__auto__){
var seq__44579__$1 = temp__5720__auto__;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__44579__$1)){
var c__4556__auto__ = cljs.core.chunk_first.call(null,seq__44579__$1);
var G__44628 = cljs.core.chunk_rest.call(null,seq__44579__$1);
var G__44629 = c__4556__auto__;
var G__44630 = cljs.core.count.call(null,c__4556__auto__);
var G__44631 = (0);
seq__44579 = G__44628;
chunk__44580 = G__44629;
count__44581 = G__44630;
i__44582 = G__44631;
continue;
} else {
var prov = cljs.core.first.call(null,seq__44579__$1);
var seq__44595_44632 = cljs.core.seq.call(null,requires);
var chunk__44596_44633 = null;
var count__44597_44634 = (0);
var i__44598_44635 = (0);
while(true){
if((i__44598_44635 < count__44597_44634)){
var req_44636 = cljs.core._nth.call(null,chunk__44596_44633,i__44598_44635);
figwheel.client.file_reloading.name_to_parent_BANG_.call(null,req_44636,prov);


var G__44637 = seq__44595_44632;
var G__44638 = chunk__44596_44633;
var G__44639 = count__44597_44634;
var G__44640 = (i__44598_44635 + (1));
seq__44595_44632 = G__44637;
chunk__44596_44633 = G__44638;
count__44597_44634 = G__44639;
i__44598_44635 = G__44640;
continue;
} else {
var temp__5720__auto___44641__$1 = cljs.core.seq.call(null,seq__44595_44632);
if(temp__5720__auto___44641__$1){
var seq__44595_44642__$1 = temp__5720__auto___44641__$1;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__44595_44642__$1)){
var c__4556__auto___44643 = cljs.core.chunk_first.call(null,seq__44595_44642__$1);
var G__44644 = cljs.core.chunk_rest.call(null,seq__44595_44642__$1);
var G__44645 = c__4556__auto___44643;
var G__44646 = cljs.core.count.call(null,c__4556__auto___44643);
var G__44647 = (0);
seq__44595_44632 = G__44644;
chunk__44596_44633 = G__44645;
count__44597_44634 = G__44646;
i__44598_44635 = G__44647;
continue;
} else {
var req_44648 = cljs.core.first.call(null,seq__44595_44642__$1);
figwheel.client.file_reloading.name_to_parent_BANG_.call(null,req_44648,prov);


var G__44649 = cljs.core.next.call(null,seq__44595_44642__$1);
var G__44650 = null;
var G__44651 = (0);
var G__44652 = (0);
seq__44595_44632 = G__44649;
chunk__44596_44633 = G__44650;
count__44597_44634 = G__44651;
i__44598_44635 = G__44652;
continue;
}
} else {
}
}
break;
}


var G__44653 = cljs.core.next.call(null,seq__44579__$1);
var G__44654 = null;
var G__44655 = (0);
var G__44656 = (0);
seq__44579 = G__44653;
chunk__44580 = G__44654;
count__44581 = G__44655;
i__44582 = G__44656;
continue;
}
} else {
return null;
}
}
break;
}
}));
}):(function (){
var requires = goog.object.filter(goog.dependencies_.requires,(function (v,k,o){
return goog.string.startsWith(k,"../");
}));
return goog.object.forEach(requires,(function (deps,path,_){
var seq__44599 = cljs.core.seq.call(null,figwheel.client.file_reloading.path__GT_name.call(null,path));
var chunk__44600 = null;
var count__44601 = (0);
var i__44602 = (0);
while(true){
if((i__44602 < count__44601)){
var prov = cljs.core._nth.call(null,chunk__44600,i__44602);
goog.object.forEach(deps,((function (seq__44599,chunk__44600,count__44601,i__44602,prov,requires){
return (function (___$1,req,___$2){
return figwheel.client.file_reloading.name_to_parent_BANG_.call(null,req,prov);
});})(seq__44599,chunk__44600,count__44601,i__44602,prov,requires))
);


var G__44657 = seq__44599;
var G__44658 = chunk__44600;
var G__44659 = count__44601;
var G__44660 = (i__44602 + (1));
seq__44599 = G__44657;
chunk__44600 = G__44658;
count__44601 = G__44659;
i__44602 = G__44660;
continue;
} else {
var temp__5720__auto__ = cljs.core.seq.call(null,seq__44599);
if(temp__5720__auto__){
var seq__44599__$1 = temp__5720__auto__;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__44599__$1)){
var c__4556__auto__ = cljs.core.chunk_first.call(null,seq__44599__$1);
var G__44661 = cljs.core.chunk_rest.call(null,seq__44599__$1);
var G__44662 = c__4556__auto__;
var G__44663 = cljs.core.count.call(null,c__4556__auto__);
var G__44664 = (0);
seq__44599 = G__44661;
chunk__44600 = G__44662;
count__44601 = G__44663;
i__44602 = G__44664;
continue;
} else {
var prov = cljs.core.first.call(null,seq__44599__$1);
goog.object.forEach(deps,((function (seq__44599,chunk__44600,count__44601,i__44602,prov,seq__44599__$1,temp__5720__auto__,requires){
return (function (___$1,req,___$2){
return figwheel.client.file_reloading.name_to_parent_BANG_.call(null,req,prov);
});})(seq__44599,chunk__44600,count__44601,i__44602,prov,seq__44599__$1,temp__5720__auto__,requires))
);


var G__44665 = cljs.core.next.call(null,seq__44599__$1);
var G__44666 = null;
var G__44667 = (0);
var G__44668 = (0);
seq__44599 = G__44665;
chunk__44600 = G__44666;
count__44601 = G__44667;
i__44602 = G__44668;
continue;
}
} else {
return null;
}
}
break;
}
}));
}));
figwheel.client.file_reloading.ns__GT_dependents = (function figwheel$client$file_reloading$ns__GT_dependents(ns){
return cljs.core.get_in.call(null,cljs.core.deref.call(null,figwheel.client.file_reloading.dependency_data),new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"dependents","dependents",136812837),ns], null));
});
figwheel.client.file_reloading.in_upper_level_QMARK_ = (function figwheel$client$file_reloading$in_upper_level_QMARK_(topo_state,current_depth,dep){
return cljs.core.some.call(null,(function (p__44669){
var vec__44670 = p__44669;
var _ = cljs.core.nth.call(null,vec__44670,(0),null);
var v = cljs.core.nth.call(null,vec__44670,(1),null);
var and__4115__auto__ = v;
if(cljs.core.truth_(and__4115__auto__)){
return v.call(null,dep);
} else {
return and__4115__auto__;
}
}),cljs.core.filter.call(null,(function (p__44673){
var vec__44674 = p__44673;
var k = cljs.core.nth.call(null,vec__44674,(0),null);
var v = cljs.core.nth.call(null,vec__44674,(1),null);
return (k > current_depth);
}),topo_state));
});
figwheel.client.file_reloading.build_topo_sort = (function figwheel$client$file_reloading$build_topo_sort(get_deps){
var get_deps__$1 = cljs.core.memoize.call(null,get_deps);
var topo_sort_helper_STAR_ = (function figwheel$client$file_reloading$build_topo_sort_$_topo_sort_helper_STAR_(x,depth,state){
var deps = get_deps__$1.call(null,x);
if(cljs.core.empty_QMARK_.call(null,deps)){
return null;
} else {
return topo_sort_STAR_.call(null,deps,depth,state);
}
});
var topo_sort_STAR_ = (function() {
var figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR_ = null;
var figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR___1 = (function (deps){
return figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR_.call(null,deps,(0),cljs.core.atom.call(null,cljs.core.sorted_map.call(null)));
});
var figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR___3 = (function (deps,depth,state){
cljs.core.swap_BANG_.call(null,state,cljs.core.update_in,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [depth], null),cljs.core.fnil.call(null,cljs.core.into,cljs.core.PersistentHashSet.EMPTY),deps);

var seq__44686_44694 = cljs.core.seq.call(null,deps);
var chunk__44687_44695 = null;
var count__44688_44696 = (0);
var i__44689_44697 = (0);
while(true){
if((i__44689_44697 < count__44688_44696)){
var dep_44698 = cljs.core._nth.call(null,chunk__44687_44695,i__44689_44697);
if(cljs.core.truth_((function (){var and__4115__auto__ = dep_44698;
if(cljs.core.truth_(and__4115__auto__)){
return cljs.core.not.call(null,figwheel.client.file_reloading.in_upper_level_QMARK_.call(null,cljs.core.deref.call(null,state),depth,dep_44698));
} else {
return and__4115__auto__;
}
})())){
topo_sort_helper_STAR_.call(null,dep_44698,(depth + (1)),state);
} else {
}


var G__44699 = seq__44686_44694;
var G__44700 = chunk__44687_44695;
var G__44701 = count__44688_44696;
var G__44702 = (i__44689_44697 + (1));
seq__44686_44694 = G__44699;
chunk__44687_44695 = G__44700;
count__44688_44696 = G__44701;
i__44689_44697 = G__44702;
continue;
} else {
var temp__5720__auto___44703 = cljs.core.seq.call(null,seq__44686_44694);
if(temp__5720__auto___44703){
var seq__44686_44704__$1 = temp__5720__auto___44703;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__44686_44704__$1)){
var c__4556__auto___44705 = cljs.core.chunk_first.call(null,seq__44686_44704__$1);
var G__44706 = cljs.core.chunk_rest.call(null,seq__44686_44704__$1);
var G__44707 = c__4556__auto___44705;
var G__44708 = cljs.core.count.call(null,c__4556__auto___44705);
var G__44709 = (0);
seq__44686_44694 = G__44706;
chunk__44687_44695 = G__44707;
count__44688_44696 = G__44708;
i__44689_44697 = G__44709;
continue;
} else {
var dep_44710 = cljs.core.first.call(null,seq__44686_44704__$1);
if(cljs.core.truth_((function (){var and__4115__auto__ = dep_44710;
if(cljs.core.truth_(and__4115__auto__)){
return cljs.core.not.call(null,figwheel.client.file_reloading.in_upper_level_QMARK_.call(null,cljs.core.deref.call(null,state),depth,dep_44710));
} else {
return and__4115__auto__;
}
})())){
topo_sort_helper_STAR_.call(null,dep_44710,(depth + (1)),state);
} else {
}


var G__44711 = cljs.core.next.call(null,seq__44686_44704__$1);
var G__44712 = null;
var G__44713 = (0);
var G__44714 = (0);
seq__44686_44694 = G__44711;
chunk__44687_44695 = G__44712;
count__44688_44696 = G__44713;
i__44689_44697 = G__44714;
continue;
}
} else {
}
}
break;
}

if(cljs.core._EQ_.call(null,depth,(0))){
return elim_dups_STAR_.call(null,cljs.core.reverse.call(null,cljs.core.vals.call(null,cljs.core.deref.call(null,state))));
} else {
return null;
}
});
figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR_ = function(deps,depth,state){
switch(arguments.length){
case 1:
return figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR___1.call(this,deps);
case 3:
return figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR___3.call(this,deps,depth,state);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR_.cljs$core$IFn$_invoke$arity$1 = figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR___1;
figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR_.cljs$core$IFn$_invoke$arity$3 = figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR___3;
return figwheel$client$file_reloading$build_topo_sort_$_topo_sort_STAR_;
})()
;
var elim_dups_STAR_ = (function figwheel$client$file_reloading$build_topo_sort_$_elim_dups_STAR_(p__44690){
var vec__44691 = p__44690;
var seq__44692 = cljs.core.seq.call(null,vec__44691);
var first__44693 = cljs.core.first.call(null,seq__44692);
var seq__44692__$1 = cljs.core.next.call(null,seq__44692);
var x = first__44693;
var xs = seq__44692__$1;
if((x == null)){
return cljs.core.List.EMPTY;
} else {
return cljs.core.cons.call(null,x,figwheel$client$file_reloading$build_topo_sort_$_elim_dups_STAR_.call(null,cljs.core.map.call(null,(function (p1__44677_SHARP_){
return clojure.set.difference.call(null,p1__44677_SHARP_,x);
}),xs)));
}
});
return topo_sort_STAR_;
});
figwheel.client.file_reloading.get_all_dependencies = (function figwheel$client$file_reloading$get_all_dependencies(ns){
var topo_sort_SINGLEQUOTE_ = figwheel.client.file_reloading.build_topo_sort.call(null,figwheel.client.file_reloading.get_requires);
return cljs.core.apply.call(null,cljs.core.concat,topo_sort_SINGLEQUOTE_.call(null,cljs.core.set.call(null,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [ns], null))));
});
figwheel.client.file_reloading.get_all_dependents = (function figwheel$client$file_reloading$get_all_dependents(nss){
var topo_sort_SINGLEQUOTE_ = figwheel.client.file_reloading.build_topo_sort.call(null,figwheel.client.file_reloading.ns__GT_dependents);
return cljs.core.filter.call(null,cljs.core.comp.call(null,cljs.core.not,figwheel.client.file_reloading.immutable_ns_QMARK_),cljs.core.reverse.call(null,cljs.core.apply.call(null,cljs.core.concat,topo_sort_SINGLEQUOTE_.call(null,cljs.core.set.call(null,nss)))));
});
figwheel.client.file_reloading.unprovide_BANG_ = ((figwheel.client.file_reloading.debug_loader_QMARK_.call(null))?(function (ns){
var path = figwheel.client.file_reloading.name__GT_path.call(null,ns);
goog.object.remove(goog.debugLoader_.written_,path);

return goog.object.remove(goog.debugLoader_.written_,[cljs.core.str.cljs$core$IFn$_invoke$arity$1(goog.basePath),cljs.core.str.cljs$core$IFn$_invoke$arity$1(path)].join(''));
}):(function (ns){
var path = figwheel.client.file_reloading.name__GT_path.call(null,ns);
goog.object.remove(goog.dependencies_.visited,path);

goog.object.remove(goog.dependencies_.written,path);

return goog.object.remove(goog.dependencies_.written,[cljs.core.str.cljs$core$IFn$_invoke$arity$1(goog.basePath),cljs.core.str.cljs$core$IFn$_invoke$arity$1(path)].join(''));
}));
figwheel.client.file_reloading.resolve_ns = ((figwheel.client.file_reloading.debug_loader_QMARK_.call(null))?figwheel.client.file_reloading.name__GT_path:(function (p1__44715_SHARP_){
return [cljs.core.str.cljs$core$IFn$_invoke$arity$1(goog.basePath),cljs.core.str.cljs$core$IFn$_invoke$arity$1(figwheel.client.file_reloading.name__GT_path.call(null,p1__44715_SHARP_))].join('');
}));
figwheel.client.file_reloading.addDependency = (function figwheel$client$file_reloading$addDependency(path,provides,requires){
var seq__44716 = cljs.core.seq.call(null,provides);
var chunk__44717 = null;
var count__44718 = (0);
var i__44719 = (0);
while(true){
if((i__44719 < count__44718)){
var prov = cljs.core._nth.call(null,chunk__44717,i__44719);
figwheel.client.file_reloading.path_to_name_BANG_.call(null,path,prov);

var seq__44728_44736 = cljs.core.seq.call(null,requires);
var chunk__44729_44737 = null;
var count__44730_44738 = (0);
var i__44731_44739 = (0);
while(true){
if((i__44731_44739 < count__44730_44738)){
var req_44740 = cljs.core._nth.call(null,chunk__44729_44737,i__44731_44739);
figwheel.client.file_reloading.name_to_parent_BANG_.call(null,req_44740,prov);


var G__44741 = seq__44728_44736;
var G__44742 = chunk__44729_44737;
var G__44743 = count__44730_44738;
var G__44744 = (i__44731_44739 + (1));
seq__44728_44736 = G__44741;
chunk__44729_44737 = G__44742;
count__44730_44738 = G__44743;
i__44731_44739 = G__44744;
continue;
} else {
var temp__5720__auto___44745 = cljs.core.seq.call(null,seq__44728_44736);
if(temp__5720__auto___44745){
var seq__44728_44746__$1 = temp__5720__auto___44745;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__44728_44746__$1)){
var c__4556__auto___44747 = cljs.core.chunk_first.call(null,seq__44728_44746__$1);
var G__44748 = cljs.core.chunk_rest.call(null,seq__44728_44746__$1);
var G__44749 = c__4556__auto___44747;
var G__44750 = cljs.core.count.call(null,c__4556__auto___44747);
var G__44751 = (0);
seq__44728_44736 = G__44748;
chunk__44729_44737 = G__44749;
count__44730_44738 = G__44750;
i__44731_44739 = G__44751;
continue;
} else {
var req_44752 = cljs.core.first.call(null,seq__44728_44746__$1);
figwheel.client.file_reloading.name_to_parent_BANG_.call(null,req_44752,prov);


var G__44753 = cljs.core.next.call(null,seq__44728_44746__$1);
var G__44754 = null;
var G__44755 = (0);
var G__44756 = (0);
seq__44728_44736 = G__44753;
chunk__44729_44737 = G__44754;
count__44730_44738 = G__44755;
i__44731_44739 = G__44756;
continue;
}
} else {
}
}
break;
}


var G__44757 = seq__44716;
var G__44758 = chunk__44717;
var G__44759 = count__44718;
var G__44760 = (i__44719 + (1));
seq__44716 = G__44757;
chunk__44717 = G__44758;
count__44718 = G__44759;
i__44719 = G__44760;
continue;
} else {
var temp__5720__auto__ = cljs.core.seq.call(null,seq__44716);
if(temp__5720__auto__){
var seq__44716__$1 = temp__5720__auto__;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__44716__$1)){
var c__4556__auto__ = cljs.core.chunk_first.call(null,seq__44716__$1);
var G__44761 = cljs.core.chunk_rest.call(null,seq__44716__$1);
var G__44762 = c__4556__auto__;
var G__44763 = cljs.core.count.call(null,c__4556__auto__);
var G__44764 = (0);
seq__44716 = G__44761;
chunk__44717 = G__44762;
count__44718 = G__44763;
i__44719 = G__44764;
continue;
} else {
var prov = cljs.core.first.call(null,seq__44716__$1);
figwheel.client.file_reloading.path_to_name_BANG_.call(null,path,prov);

var seq__44732_44765 = cljs.core.seq.call(null,requires);
var chunk__44733_44766 = null;
var count__44734_44767 = (0);
var i__44735_44768 = (0);
while(true){
if((i__44735_44768 < count__44734_44767)){
var req_44769 = cljs.core._nth.call(null,chunk__44733_44766,i__44735_44768);
figwheel.client.file_reloading.name_to_parent_BANG_.call(null,req_44769,prov);


var G__44770 = seq__44732_44765;
var G__44771 = chunk__44733_44766;
var G__44772 = count__44734_44767;
var G__44773 = (i__44735_44768 + (1));
seq__44732_44765 = G__44770;
chunk__44733_44766 = G__44771;
count__44734_44767 = G__44772;
i__44735_44768 = G__44773;
continue;
} else {
var temp__5720__auto___44774__$1 = cljs.core.seq.call(null,seq__44732_44765);
if(temp__5720__auto___44774__$1){
var seq__44732_44775__$1 = temp__5720__auto___44774__$1;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__44732_44775__$1)){
var c__4556__auto___44776 = cljs.core.chunk_first.call(null,seq__44732_44775__$1);
var G__44777 = cljs.core.chunk_rest.call(null,seq__44732_44775__$1);
var G__44778 = c__4556__auto___44776;
var G__44779 = cljs.core.count.call(null,c__4556__auto___44776);
var G__44780 = (0);
seq__44732_44765 = G__44777;
chunk__44733_44766 = G__44778;
count__44734_44767 = G__44779;
i__44735_44768 = G__44780;
continue;
} else {
var req_44781 = cljs.core.first.call(null,seq__44732_44775__$1);
figwheel.client.file_reloading.name_to_parent_BANG_.call(null,req_44781,prov);


var G__44782 = cljs.core.next.call(null,seq__44732_44775__$1);
var G__44783 = null;
var G__44784 = (0);
var G__44785 = (0);
seq__44732_44765 = G__44782;
chunk__44733_44766 = G__44783;
count__44734_44767 = G__44784;
i__44735_44768 = G__44785;
continue;
}
} else {
}
}
break;
}


var G__44786 = cljs.core.next.call(null,seq__44716__$1);
var G__44787 = null;
var G__44788 = (0);
var G__44789 = (0);
seq__44716 = G__44786;
chunk__44717 = G__44787;
count__44718 = G__44788;
i__44719 = G__44789;
continue;
}
} else {
return null;
}
}
break;
}
});
figwheel.client.file_reloading.figwheel_require = (function figwheel$client$file_reloading$figwheel_require(src,reload){
(goog.require = figwheel.client.file_reloading.figwheel_require);

if(cljs.core._EQ_.call(null,reload,"reload-all")){
var seq__44790_44794 = cljs.core.seq.call(null,figwheel.client.file_reloading.get_all_dependencies.call(null,src));
var chunk__44791_44795 = null;
var count__44792_44796 = (0);
var i__44793_44797 = (0);
while(true){
if((i__44793_44797 < count__44792_44796)){
var ns_44798 = cljs.core._nth.call(null,chunk__44791_44795,i__44793_44797);
figwheel.client.file_reloading.unprovide_BANG_.call(null,ns_44798);


var G__44799 = seq__44790_44794;
var G__44800 = chunk__44791_44795;
var G__44801 = count__44792_44796;
var G__44802 = (i__44793_44797 + (1));
seq__44790_44794 = G__44799;
chunk__44791_44795 = G__44800;
count__44792_44796 = G__44801;
i__44793_44797 = G__44802;
continue;
} else {
var temp__5720__auto___44803 = cljs.core.seq.call(null,seq__44790_44794);
if(temp__5720__auto___44803){
var seq__44790_44804__$1 = temp__5720__auto___44803;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__44790_44804__$1)){
var c__4556__auto___44805 = cljs.core.chunk_first.call(null,seq__44790_44804__$1);
var G__44806 = cljs.core.chunk_rest.call(null,seq__44790_44804__$1);
var G__44807 = c__4556__auto___44805;
var G__44808 = cljs.core.count.call(null,c__4556__auto___44805);
var G__44809 = (0);
seq__44790_44794 = G__44806;
chunk__44791_44795 = G__44807;
count__44792_44796 = G__44808;
i__44793_44797 = G__44809;
continue;
} else {
var ns_44810 = cljs.core.first.call(null,seq__44790_44804__$1);
figwheel.client.file_reloading.unprovide_BANG_.call(null,ns_44810);


var G__44811 = cljs.core.next.call(null,seq__44790_44804__$1);
var G__44812 = null;
var G__44813 = (0);
var G__44814 = (0);
seq__44790_44794 = G__44811;
chunk__44791_44795 = G__44812;
count__44792_44796 = G__44813;
i__44793_44797 = G__44814;
continue;
}
} else {
}
}
break;
}
} else {
}

if(cljs.core.truth_(reload)){
figwheel.client.file_reloading.unprovide_BANG_.call(null,src);
} else {
}

return goog.require_figwheel_backup_(src);
});
/**
 * Reusable browser REPL bootstrapping. Patches the essential functions
 *   in goog.base to support re-loading of namespaces after page load.
 */
figwheel.client.file_reloading.bootstrap_goog_base = (function figwheel$client$file_reloading$bootstrap_goog_base(){
if(cljs.core.truth_(COMPILED)){
return null;
} else {
(goog.require_figwheel_backup_ = (function (){var or__4126__auto__ = goog.require__;
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return goog.require;
}
})());

(goog.isProvided_ = (function (name){
return false;
}));

figwheel.client.file_reloading.setup_path__GT_name_BANG_.call(null);

figwheel.client.file_reloading.setup_ns__GT_dependents_BANG_.call(null);

(goog.addDependency_figwheel_backup_ = goog.addDependency);

(goog.addDependency = (function() { 
var G__44815__delegate = function (args){
cljs.core.apply.call(null,figwheel.client.file_reloading.addDependency,args);

return cljs.core.apply.call(null,goog.addDependency_figwheel_backup_,args);
};
var G__44815 = function (var_args){
var args = null;
if (arguments.length > 0) {
var G__44816__i = 0, G__44816__a = new Array(arguments.length -  0);
while (G__44816__i < G__44816__a.length) {G__44816__a[G__44816__i] = arguments[G__44816__i + 0]; ++G__44816__i;}
  args = new cljs.core.IndexedSeq(G__44816__a,0,null);
} 
return G__44815__delegate.call(this,args);};
G__44815.cljs$lang$maxFixedArity = 0;
G__44815.cljs$lang$applyTo = (function (arglist__44817){
var args = cljs.core.seq(arglist__44817);
return G__44815__delegate(args);
});
G__44815.cljs$core$IFn$_invoke$arity$variadic = G__44815__delegate;
return G__44815;
})()
);

goog.constructNamespace_("cljs.user");

(goog.global.CLOSURE_IMPORT_SCRIPT = figwheel.client.file_reloading.queued_file_reload);

return (goog.require = figwheel.client.file_reloading.figwheel_require);
}
});
figwheel.client.file_reloading.patch_goog_base = (function figwheel$client$file_reloading$patch_goog_base(){
if((typeof figwheel !== 'undefined') && (typeof figwheel.client !== 'undefined') && (typeof figwheel.client.file_reloading !== 'undefined') && (typeof figwheel.client.file_reloading.bootstrapped_cljs !== 'undefined')){
return null;
} else {
return (
figwheel.client.file_reloading.bootstrapped_cljs = (function (){
figwheel.client.file_reloading.bootstrap_goog_base.call(null);

return true;
})()
)
;
}
});
figwheel.client.file_reloading.gloader = (((typeof goog !== 'undefined') && (typeof goog.net !== 'undefined') && (typeof goog.net.jsloader !== 'undefined') && (typeof goog.net.jsloader.safeLoad !== 'undefined'))?(function (p1__44818_SHARP_,p2__44819_SHARP_){
return goog.net.jsloader.safeLoad(goog.html.legacyconversions.trustedResourceUrlFromString(cljs.core.str.cljs$core$IFn$_invoke$arity$1(p1__44818_SHARP_)),p2__44819_SHARP_);
}):(((typeof goog !== 'undefined') && (typeof goog.net !== 'undefined') && (typeof goog.net.jsloader !== 'undefined') && (typeof goog.net.jsloader.load !== 'undefined'))?(function (p1__44820_SHARP_,p2__44821_SHARP_){
return goog.net.jsloader.load(cljs.core.str.cljs$core$IFn$_invoke$arity$1(p1__44820_SHARP_),p2__44821_SHARP_);
}):(function(){throw cljs.core.ex_info.call(null,"No remote script loading function found.",cljs.core.PersistentArrayMap.EMPTY)})()
));
figwheel.client.file_reloading.reload_file_in_html_env = (function figwheel$client$file_reloading$reload_file_in_html_env(request_url,callback){

var G__44822 = figwheel.client.file_reloading.gloader.call(null,figwheel.client.file_reloading.add_cache_buster.call(null,request_url),({"cleanupWhenDone": true}));
G__44822.addCallback((function (){
return cljs.core.apply.call(null,callback,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [true], null));
}));

G__44822.addErrback((function (){
return cljs.core.apply.call(null,callback,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [false], null));
}));

return G__44822;
});
figwheel.client.file_reloading.write_script_tag_import = figwheel.client.file_reloading.reload_file_in_html_env;
goog.exportSymbol('figwheel.client.file_reloading.write_script_tag_import', figwheel.client.file_reloading.write_script_tag_import);
figwheel.client.file_reloading.worker_import_script = (function figwheel$client$file_reloading$worker_import_script(request_url,callback){

return callback.call(null,(function (){try{self.importScripts(figwheel.client.file_reloading.add_cache_buster.call(null,request_url));

return true;
}catch (e44823){if((e44823 instanceof Error)){
var e = e44823;
figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"error","error",-978969032),["Figwheel: Error loading file ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(request_url)].join(''));

figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"error","error",-978969032),e.stack);

return false;
} else {
throw e44823;

}
}})());
});
goog.exportSymbol('figwheel.client.file_reloading.worker_import_script', figwheel.client.file_reloading.worker_import_script);
figwheel.client.file_reloading.create_node_script_import_fn = (function figwheel$client$file_reloading$create_node_script_import_fn(){
var node_path_lib = require("path");
var util_pattern = [cljs.core.str.cljs$core$IFn$_invoke$arity$1(node_path_lib.sep),cljs.core.str.cljs$core$IFn$_invoke$arity$1(node_path_lib.join("goog","bootstrap","nodejs.js"))].join('');
var util_path = goog.object.findKey(require.cache,(function (v,k,o){
return goog.string.endsWith(k,util_pattern);
}));
var parts = cljs.core.pop.call(null,cljs.core.pop.call(null,clojure.string.split.call(null,util_path,/[\/\\]/)));
var root_path = clojure.string.join.call(null,node_path_lib.sep,parts);
return (function (request_url,callback){

var cache_path = node_path_lib.resolve(root_path,request_url);
goog.object.remove(require.cache,cache_path);

return callback.call(null,(function (){try{return require(cache_path);
}catch (e44824){if((e44824 instanceof Error)){
var e = e44824;
figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"error","error",-978969032),["Figwheel: Error loading file ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(cache_path)].join(''));

figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"error","error",-978969032),e.stack);

return false;
} else {
throw e44824;

}
}})());
});
});
goog.exportSymbol('figwheel.client.file_reloading.create_node_script_import_fn', figwheel.client.file_reloading.create_node_script_import_fn);
figwheel.client.file_reloading.reload_file_STAR_ = (function (){var pred__44825 = cljs.core._EQ_;
var expr__44826 = figwheel.client.utils.host_env_QMARK_.call(null);
if(cljs.core.truth_(pred__44825.call(null,new cljs.core.Keyword(null,"node","node",581201198),expr__44826))){
return figwheel.client.file_reloading.create_node_script_import_fn.call(null);
} else {
if(cljs.core.truth_(pred__44825.call(null,new cljs.core.Keyword(null,"html","html",-998796897),expr__44826))){
return figwheel.client.file_reloading.write_script_tag_import;
} else {
if(cljs.core.truth_(pred__44825.call(null,new cljs.core.Keyword(null,"worker","worker",938239996),expr__44826))){
return figwheel.client.file_reloading.worker_import_script;
} else {
return (function (a,b){
throw "Reload not defined for this platform";
});
}
}
}
})();
figwheel.client.file_reloading.reload_file = (function figwheel$client$file_reloading$reload_file(p__44828,callback){
var map__44829 = p__44828;
var map__44829__$1 = (((((!((map__44829 == null))))?(((((map__44829.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__44829.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__44829):map__44829);
var file_msg = map__44829__$1;
var request_url = cljs.core.get.call(null,map__44829__$1,new cljs.core.Keyword(null,"request-url","request-url",2100346596));

figwheel.client.utils.debug_prn.call(null,["FigWheel: Attempting to load ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(request_url)].join(''));

return (function (){var or__4126__auto__ = goog.object.get(goog.global,"FIGWHEEL_IMPORT_SCRIPT");
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return figwheel.client.file_reloading.reload_file_STAR_;
}
})().call(null,request_url,(function (success_QMARK_){
if(cljs.core.truth_(success_QMARK_)){
figwheel.client.utils.debug_prn.call(null,["FigWheel: Successfully loaded ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(request_url)].join(''));

return cljs.core.apply.call(null,callback,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [cljs.core.assoc.call(null,file_msg,new cljs.core.Keyword(null,"loaded-file","loaded-file",-168399375),true)], null));
} else {
figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"error","error",-978969032),["Figwheel: Error loading file ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(request_url)].join(''));

return cljs.core.apply.call(null,callback,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [file_msg], null));
}
}));
});
if((typeof figwheel !== 'undefined') && (typeof figwheel.client !== 'undefined') && (typeof figwheel.client.file_reloading !== 'undefined') && (typeof figwheel.client.file_reloading.reload_chan !== 'undefined')){
} else {
figwheel.client.file_reloading.reload_chan = cljs.core.async.chan.call(null);
}
if((typeof figwheel !== 'undefined') && (typeof figwheel.client !== 'undefined') && (typeof figwheel.client.file_reloading !== 'undefined') && (typeof figwheel.client.file_reloading.on_load_callbacks !== 'undefined')){
} else {
figwheel.client.file_reloading.on_load_callbacks = cljs.core.atom.call(null,cljs.core.PersistentArrayMap.EMPTY);
}
if((typeof figwheel !== 'undefined') && (typeof figwheel.client !== 'undefined') && (typeof figwheel.client.file_reloading !== 'undefined') && (typeof figwheel.client.file_reloading.dependencies_loaded !== 'undefined')){
} else {
figwheel.client.file_reloading.dependencies_loaded = cljs.core.atom.call(null,cljs.core.PersistentVector.EMPTY);
}
figwheel.client.file_reloading.blocking_load = (function figwheel$client$file_reloading$blocking_load(url){
var out = cljs.core.async.chan.call(null);
figwheel.client.file_reloading.reload_file.call(null,new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"request-url","request-url",2100346596),url], null),(function (file_msg){
cljs.core.async.put_BANG_.call(null,out,file_msg);

return cljs.core.async.close_BANG_.call(null,out);
}));

return out;
});
if((typeof figwheel !== 'undefined') && (typeof figwheel.client !== 'undefined') && (typeof figwheel.client.file_reloading !== 'undefined') && (typeof figwheel.client.file_reloading.reloader_loop !== 'undefined')){
} else {
figwheel.client.file_reloading.reloader_loop = (function (){var c__26800__auto__ = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_44867){
var state_val_44868 = (state_44867[(1)]);
if((state_val_44868 === (7))){
var inst_44863 = (state_44867[(2)]);
var state_44867__$1 = state_44867;
var statearr_44869_44895 = state_44867__$1;
(statearr_44869_44895[(2)] = inst_44863);

(statearr_44869_44895[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (1))){
var state_44867__$1 = state_44867;
var statearr_44870_44896 = state_44867__$1;
(statearr_44870_44896[(2)] = null);

(statearr_44870_44896[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (4))){
var inst_44833 = (state_44867[(7)]);
var inst_44833__$1 = (state_44867[(2)]);
var state_44867__$1 = (function (){var statearr_44871 = state_44867;
(statearr_44871[(7)] = inst_44833__$1);

return statearr_44871;
})();
if(cljs.core.truth_(inst_44833__$1)){
var statearr_44872_44897 = state_44867__$1;
(statearr_44872_44897[(1)] = (5));

} else {
var statearr_44873_44898 = state_44867__$1;
(statearr_44873_44898[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (15))){
var inst_44848 = (state_44867[(8)]);
var inst_44846 = (state_44867[(9)]);
var inst_44850 = inst_44848.call(null,inst_44846);
var state_44867__$1 = state_44867;
var statearr_44874_44899 = state_44867__$1;
(statearr_44874_44899[(2)] = inst_44850);

(statearr_44874_44899[(1)] = (17));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (13))){
var inst_44857 = (state_44867[(2)]);
var state_44867__$1 = state_44867;
var statearr_44875_44900 = state_44867__$1;
(statearr_44875_44900[(2)] = inst_44857);

(statearr_44875_44900[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (6))){
var state_44867__$1 = state_44867;
var statearr_44876_44901 = state_44867__$1;
(statearr_44876_44901[(2)] = null);

(statearr_44876_44901[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (17))){
var inst_44854 = (state_44867[(2)]);
var state_44867__$1 = state_44867;
var statearr_44877_44902 = state_44867__$1;
(statearr_44877_44902[(2)] = inst_44854);

(statearr_44877_44902[(1)] = (13));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (3))){
var inst_44865 = (state_44867[(2)]);
var state_44867__$1 = state_44867;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_44867__$1,inst_44865);
} else {
if((state_val_44868 === (12))){
var state_44867__$1 = state_44867;
var statearr_44878_44903 = state_44867__$1;
(statearr_44878_44903[(2)] = null);

(statearr_44878_44903[(1)] = (13));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (2))){
var state_44867__$1 = state_44867;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_44867__$1,(4),figwheel.client.file_reloading.reload_chan);
} else {
if((state_val_44868 === (11))){
var inst_44838 = (state_44867[(10)]);
var inst_44844 = figwheel.client.file_reloading.blocking_load.call(null,inst_44838);
var state_44867__$1 = state_44867;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_44867__$1,(14),inst_44844);
} else {
if((state_val_44868 === (9))){
var inst_44838 = (state_44867[(10)]);
var state_44867__$1 = state_44867;
if(cljs.core.truth_(inst_44838)){
var statearr_44879_44904 = state_44867__$1;
(statearr_44879_44904[(1)] = (11));

} else {
var statearr_44880_44905 = state_44867__$1;
(statearr_44880_44905[(1)] = (12));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (5))){
var inst_44839 = (state_44867[(11)]);
var inst_44833 = (state_44867[(7)]);
var inst_44838 = cljs.core.nth.call(null,inst_44833,(0),null);
var inst_44839__$1 = cljs.core.nth.call(null,inst_44833,(1),null);
var state_44867__$1 = (function (){var statearr_44881 = state_44867;
(statearr_44881[(11)] = inst_44839__$1);

(statearr_44881[(10)] = inst_44838);

return statearr_44881;
})();
if(cljs.core.truth_(inst_44839__$1)){
var statearr_44882_44906 = state_44867__$1;
(statearr_44882_44906[(1)] = (8));

} else {
var statearr_44883_44907 = state_44867__$1;
(statearr_44883_44907[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (14))){
var inst_44838 = (state_44867[(10)]);
var inst_44848 = (state_44867[(8)]);
var inst_44846 = (state_44867[(2)]);
var inst_44847 = cljs.core.deref.call(null,figwheel.client.file_reloading.on_load_callbacks);
var inst_44848__$1 = cljs.core.get.call(null,inst_44847,inst_44838);
var state_44867__$1 = (function (){var statearr_44884 = state_44867;
(statearr_44884[(8)] = inst_44848__$1);

(statearr_44884[(9)] = inst_44846);

return statearr_44884;
})();
if(cljs.core.truth_(inst_44848__$1)){
var statearr_44885_44908 = state_44867__$1;
(statearr_44885_44908[(1)] = (15));

} else {
var statearr_44886_44909 = state_44867__$1;
(statearr_44886_44909[(1)] = (16));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (16))){
var inst_44846 = (state_44867[(9)]);
var inst_44852 = cljs.core.swap_BANG_.call(null,figwheel.client.file_reloading.dependencies_loaded,cljs.core.conj,inst_44846);
var state_44867__$1 = state_44867;
var statearr_44887_44910 = state_44867__$1;
(statearr_44887_44910[(2)] = inst_44852);

(statearr_44887_44910[(1)] = (17));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (10))){
var inst_44859 = (state_44867[(2)]);
var state_44867__$1 = (function (){var statearr_44888 = state_44867;
(statearr_44888[(12)] = inst_44859);

return statearr_44888;
})();
var statearr_44889_44911 = state_44867__$1;
(statearr_44889_44911[(2)] = null);

(statearr_44889_44911[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44868 === (8))){
var inst_44839 = (state_44867[(11)]);
var inst_44841 = eval(inst_44839);
var state_44867__$1 = state_44867;
var statearr_44890_44912 = state_44867__$1;
(statearr_44890_44912[(2)] = inst_44841);

(statearr_44890_44912[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
return null;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
});
return (function() {
var figwheel$client$file_reloading$state_machine__26706__auto__ = null;
var figwheel$client$file_reloading$state_machine__26706__auto____0 = (function (){
var statearr_44891 = [null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_44891[(0)] = figwheel$client$file_reloading$state_machine__26706__auto__);

(statearr_44891[(1)] = (1));

return statearr_44891;
});
var figwheel$client$file_reloading$state_machine__26706__auto____1 = (function (state_44867){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_44867);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e44892){if((e44892 instanceof Object)){
var ex__26709__auto__ = e44892;
var statearr_44893_44913 = state_44867;
(statearr_44893_44913[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_44867);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e44892;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__44914 = state_44867;
state_44867 = G__44914;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
figwheel$client$file_reloading$state_machine__26706__auto__ = function(state_44867){
switch(arguments.length){
case 0:
return figwheel$client$file_reloading$state_machine__26706__auto____0.call(this);
case 1:
return figwheel$client$file_reloading$state_machine__26706__auto____1.call(this,state_44867);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
figwheel$client$file_reloading$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = figwheel$client$file_reloading$state_machine__26706__auto____0;
figwheel$client$file_reloading$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = figwheel$client$file_reloading$state_machine__26706__auto____1;
return figwheel$client$file_reloading$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_44894 = f__26801__auto__.call(null);
(statearr_44894[(6)] = c__26800__auto__);

return statearr_44894;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));

return c__26800__auto__;
})();
}
figwheel.client.file_reloading.queued_file_reload = (function figwheel$client$file_reloading$queued_file_reload(var_args){
var G__44916 = arguments.length;
switch (G__44916) {
case 1:
return figwheel.client.file_reloading.queued_file_reload.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 2:
return figwheel.client.file_reloading.queued_file_reload.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(figwheel.client.file_reloading.queued_file_reload.cljs$core$IFn$_invoke$arity$1 = (function (url){
return figwheel.client.file_reloading.queued_file_reload.call(null,url,null);
}));

(figwheel.client.file_reloading.queued_file_reload.cljs$core$IFn$_invoke$arity$2 = (function (url,opt_source_text){
return cljs.core.async.put_BANG_.call(null,figwheel.client.file_reloading.reload_chan,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [url,opt_source_text], null));
}));

(figwheel.client.file_reloading.queued_file_reload.cljs$lang$maxFixedArity = 2);

figwheel.client.file_reloading.require_with_callback = (function figwheel$client$file_reloading$require_with_callback(p__44918,callback){
var map__44919 = p__44918;
var map__44919__$1 = (((((!((map__44919 == null))))?(((((map__44919.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__44919.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__44919):map__44919);
var file_msg = map__44919__$1;
var namespace = cljs.core.get.call(null,map__44919__$1,new cljs.core.Keyword(null,"namespace","namespace",-377510372));
var request_url = figwheel.client.file_reloading.resolve_ns.call(null,namespace);
cljs.core.swap_BANG_.call(null,figwheel.client.file_reloading.on_load_callbacks,cljs.core.assoc,request_url,(function (file_msg_SINGLEQUOTE_){
cljs.core.swap_BANG_.call(null,figwheel.client.file_reloading.on_load_callbacks,cljs.core.dissoc,request_url);

return cljs.core.apply.call(null,callback,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [cljs.core.merge.call(null,file_msg,cljs.core.select_keys.call(null,file_msg_SINGLEQUOTE_,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"loaded-file","loaded-file",-168399375)], null)))], null));
}));

return figwheel.client.file_reloading.figwheel_require.call(null,cljs.core.name.call(null,namespace),true);
});
figwheel.client.file_reloading.figwheel_no_load_QMARK_ = (function figwheel$client$file_reloading$figwheel_no_load_QMARK_(p__44921){
var map__44922 = p__44921;
var map__44922__$1 = (((((!((map__44922 == null))))?(((((map__44922.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__44922.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__44922):map__44922);
var file_msg = map__44922__$1;
var namespace = cljs.core.get.call(null,map__44922__$1,new cljs.core.Keyword(null,"namespace","namespace",-377510372));
var meta_pragmas = cljs.core.get.call(null,cljs.core.deref.call(null,figwheel.client.file_reloading.figwheel_meta_pragmas),cljs.core.name.call(null,namespace));
return new cljs.core.Keyword(null,"figwheel-no-load","figwheel-no-load",-555840179).cljs$core$IFn$_invoke$arity$1(meta_pragmas);
});
figwheel.client.file_reloading.ns_exists_QMARK_ = (function figwheel$client$file_reloading$ns_exists_QMARK_(namespace){
return (!((cljs.core.reduce.call(null,cljs.core.fnil.call(null,goog.object.get,({})),goog.global,clojure.string.split.call(null,cljs.core.name.call(null,namespace),".")) == null)));
});
figwheel.client.file_reloading.reload_file_QMARK_ = (function figwheel$client$file_reloading$reload_file_QMARK_(p__44924){
var map__44925 = p__44924;
var map__44925__$1 = (((((!((map__44925 == null))))?(((((map__44925.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__44925.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__44925):map__44925);
var file_msg = map__44925__$1;
var namespace = cljs.core.get.call(null,map__44925__$1,new cljs.core.Keyword(null,"namespace","namespace",-377510372));

var meta_pragmas = cljs.core.get.call(null,cljs.core.deref.call(null,figwheel.client.file_reloading.figwheel_meta_pragmas),cljs.core.name.call(null,namespace));
if(cljs.core.not.call(null,figwheel.client.file_reloading.figwheel_no_load_QMARK_.call(null,file_msg))){
var or__4126__auto__ = new cljs.core.Keyword(null,"figwheel-always","figwheel-always",799819691).cljs$core$IFn$_invoke$arity$1(meta_pragmas);
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
var or__4126__auto____$1 = new cljs.core.Keyword(null,"figwheel-load","figwheel-load",1316089175).cljs$core$IFn$_invoke$arity$1(meta_pragmas);
if(cljs.core.truth_(or__4126__auto____$1)){
return or__4126__auto____$1;
} else {
var or__4126__auto____$2 = figwheel.client.file_reloading.provided_QMARK_.call(null,cljs.core.name.call(null,namespace));
if(cljs.core.truth_(or__4126__auto____$2)){
return or__4126__auto____$2;
} else {
return figwheel.client.file_reloading.ns_exists_QMARK_.call(null,namespace);
}
}
}
} else {
return false;
}
});
figwheel.client.file_reloading.js_reload = (function figwheel$client$file_reloading$js_reload(p__44927,callback){
var map__44928 = p__44927;
var map__44928__$1 = (((((!((map__44928 == null))))?(((((map__44928.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__44928.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__44928):map__44928);
var file_msg = map__44928__$1;
var request_url = cljs.core.get.call(null,map__44928__$1,new cljs.core.Keyword(null,"request-url","request-url",2100346596));
var namespace = cljs.core.get.call(null,map__44928__$1,new cljs.core.Keyword(null,"namespace","namespace",-377510372));

if(cljs.core.truth_(figwheel.client.file_reloading.reload_file_QMARK_.call(null,file_msg))){
return figwheel.client.file_reloading.require_with_callback.call(null,file_msg,callback);
} else {
figwheel.client.utils.debug_prn.call(null,["Figwheel: Not trying to load file ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(request_url)].join(''));

return cljs.core.apply.call(null,callback,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [file_msg], null));
}
});
figwheel.client.file_reloading.reload_js_file = (function figwheel$client$file_reloading$reload_js_file(file_msg){
var out = cljs.core.async.chan.call(null);
figwheel.client.file_reloading.js_reload.call(null,file_msg,(function (url){
cljs.core.async.put_BANG_.call(null,out,url);

return cljs.core.async.close_BANG_.call(null,out);
}));

return out;
});
/**
 * Returns a chanel with one collection of loaded filenames on it.
 */
figwheel.client.file_reloading.load_all_js_files = (function figwheel$client$file_reloading$load_all_js_files(files){
var out = cljs.core.async.chan.call(null);
var c__26800__auto___44978 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_44963){
var state_val_44964 = (state_44963[(1)]);
if((state_val_44964 === (1))){
var inst_44937 = cljs.core.seq.call(null,files);
var inst_44938 = cljs.core.first.call(null,inst_44937);
var inst_44939 = cljs.core.next.call(null,inst_44937);
var inst_44940 = files;
var state_44963__$1 = (function (){var statearr_44965 = state_44963;
(statearr_44965[(7)] = inst_44940);

(statearr_44965[(8)] = inst_44938);

(statearr_44965[(9)] = inst_44939);

return statearr_44965;
})();
var statearr_44966_44979 = state_44963__$1;
(statearr_44966_44979[(2)] = null);

(statearr_44966_44979[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44964 === (2))){
var inst_44940 = (state_44963[(7)]);
var inst_44946 = (state_44963[(10)]);
var inst_44945 = cljs.core.seq.call(null,inst_44940);
var inst_44946__$1 = cljs.core.first.call(null,inst_44945);
var inst_44947 = cljs.core.next.call(null,inst_44945);
var inst_44948 = (inst_44946__$1 == null);
var inst_44949 = cljs.core.not.call(null,inst_44948);
var state_44963__$1 = (function (){var statearr_44967 = state_44963;
(statearr_44967[(10)] = inst_44946__$1);

(statearr_44967[(11)] = inst_44947);

return statearr_44967;
})();
if(inst_44949){
var statearr_44968_44980 = state_44963__$1;
(statearr_44968_44980[(1)] = (4));

} else {
var statearr_44969_44981 = state_44963__$1;
(statearr_44969_44981[(1)] = (5));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44964 === (3))){
var inst_44961 = (state_44963[(2)]);
var state_44963__$1 = state_44963;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_44963__$1,inst_44961);
} else {
if((state_val_44964 === (4))){
var inst_44946 = (state_44963[(10)]);
var inst_44951 = figwheel.client.file_reloading.reload_js_file.call(null,inst_44946);
var state_44963__$1 = state_44963;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_44963__$1,(7),inst_44951);
} else {
if((state_val_44964 === (5))){
var inst_44957 = cljs.core.async.close_BANG_.call(null,out);
var state_44963__$1 = state_44963;
var statearr_44970_44982 = state_44963__$1;
(statearr_44970_44982[(2)] = inst_44957);

(statearr_44970_44982[(1)] = (6));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44964 === (6))){
var inst_44959 = (state_44963[(2)]);
var state_44963__$1 = state_44963;
var statearr_44971_44983 = state_44963__$1;
(statearr_44971_44983[(2)] = inst_44959);

(statearr_44971_44983[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44964 === (7))){
var inst_44947 = (state_44963[(11)]);
var inst_44953 = (state_44963[(2)]);
var inst_44954 = cljs.core.async.put_BANG_.call(null,out,inst_44953);
var inst_44940 = inst_44947;
var state_44963__$1 = (function (){var statearr_44972 = state_44963;
(statearr_44972[(7)] = inst_44940);

(statearr_44972[(12)] = inst_44954);

return statearr_44972;
})();
var statearr_44973_44984 = state_44963__$1;
(statearr_44973_44984[(2)] = null);

(statearr_44973_44984[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
return null;
}
}
}
}
}
}
}
});
return (function() {
var figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto__ = null;
var figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto____0 = (function (){
var statearr_44974 = [null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_44974[(0)] = figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto__);

(statearr_44974[(1)] = (1));

return statearr_44974;
});
var figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto____1 = (function (state_44963){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_44963);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e44975){if((e44975 instanceof Object)){
var ex__26709__auto__ = e44975;
var statearr_44976_44985 = state_44963;
(statearr_44976_44985[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_44963);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e44975;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__44986 = state_44963;
state_44963 = G__44986;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto__ = function(state_44963){
switch(arguments.length){
case 0:
return figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto____0.call(this);
case 1:
return figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto____1.call(this,state_44963);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto____0;
figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto____1;
return figwheel$client$file_reloading$load_all_js_files_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_44977 = f__26801__auto__.call(null);
(statearr_44977[(6)] = c__26800__auto___44978);

return statearr_44977;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return cljs.core.async.into.call(null,cljs.core.PersistentVector.EMPTY,out);
});
figwheel.client.file_reloading.eval_body = (function figwheel$client$file_reloading$eval_body(p__44987,opts){
var map__44988 = p__44987;
var map__44988__$1 = (((((!((map__44988 == null))))?(((((map__44988.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__44988.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__44988):map__44988);
var eval_body = cljs.core.get.call(null,map__44988__$1,new cljs.core.Keyword(null,"eval-body","eval-body",-907279883));
var file = cljs.core.get.call(null,map__44988__$1,new cljs.core.Keyword(null,"file","file",-1269645878));
if(cljs.core.truth_((function (){var and__4115__auto__ = eval_body;
if(cljs.core.truth_(and__4115__auto__)){
return typeof eval_body === 'string';
} else {
return and__4115__auto__;
}
})())){
var code = eval_body;
try{figwheel.client.utils.debug_prn.call(null,["Evaling file ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(file)].join(''));

return figwheel.client.utils.eval_helper.call(null,code,opts);
}catch (e44990){var e = e44990;
return figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"error","error",-978969032),["Unable to evaluate ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(file)].join(''));
}} else {
return null;
}
});
figwheel.client.file_reloading.expand_files = (function figwheel$client$file_reloading$expand_files(files){
var deps = figwheel.client.file_reloading.get_all_dependents.call(null,cljs.core.map.call(null,new cljs.core.Keyword(null,"namespace","namespace",-377510372),files));
return cljs.core.filter.call(null,cljs.core.comp.call(null,cljs.core.not,cljs.core.partial.call(null,cljs.core.re_matches,/figwheel\.connect.*/),new cljs.core.Keyword(null,"namespace","namespace",-377510372)),cljs.core.map.call(null,(function (n){
var temp__5718__auto__ = cljs.core.first.call(null,cljs.core.filter.call(null,(function (p1__44991_SHARP_){
return cljs.core._EQ_.call(null,new cljs.core.Keyword(null,"namespace","namespace",-377510372).cljs$core$IFn$_invoke$arity$1(p1__44991_SHARP_),n);
}),files));
if(cljs.core.truth_(temp__5718__auto__)){
var file_msg = temp__5718__auto__;
return file_msg;
} else {
return new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"type","type",1174270348),new cljs.core.Keyword(null,"namespace","namespace",-377510372),new cljs.core.Keyword(null,"namespace","namespace",-377510372),n], null);
}
}),deps));
});
figwheel.client.file_reloading.sort_files = (function figwheel$client$file_reloading$sort_files(files){
if((cljs.core.count.call(null,files) <= (1))){
return files;
} else {
var keep_files = cljs.core.set.call(null,cljs.core.keep.call(null,new cljs.core.Keyword(null,"namespace","namespace",-377510372),files));
return cljs.core.filter.call(null,cljs.core.comp.call(null,keep_files,new cljs.core.Keyword(null,"namespace","namespace",-377510372)),figwheel.client.file_reloading.expand_files.call(null,files));
}
});
figwheel.client.file_reloading.get_figwheel_always = (function figwheel$client$file_reloading$get_figwheel_always(){
return cljs.core.map.call(null,(function (p__44992){
var vec__44993 = p__44992;
var k = cljs.core.nth.call(null,vec__44993,(0),null);
var v = cljs.core.nth.call(null,vec__44993,(1),null);
return new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"namespace","namespace",-377510372),k,new cljs.core.Keyword(null,"type","type",1174270348),new cljs.core.Keyword(null,"namespace","namespace",-377510372)], null);
}),cljs.core.filter.call(null,(function (p__44996){
var vec__44997 = p__44996;
var k = cljs.core.nth.call(null,vec__44997,(0),null);
var v = cljs.core.nth.call(null,vec__44997,(1),null);
return new cljs.core.Keyword(null,"figwheel-always","figwheel-always",799819691).cljs$core$IFn$_invoke$arity$1(v);
}),cljs.core.deref.call(null,figwheel.client.file_reloading.figwheel_meta_pragmas)));
});
figwheel.client.file_reloading.reload_js_files = (function figwheel$client$file_reloading$reload_js_files(p__45003,p__45004){
var map__45005 = p__45003;
var map__45005__$1 = (((((!((map__45005 == null))))?(((((map__45005.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__45005.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__45005):map__45005);
var opts = map__45005__$1;
var before_jsload = cljs.core.get.call(null,map__45005__$1,new cljs.core.Keyword(null,"before-jsload","before-jsload",-847513128));
var on_jsload = cljs.core.get.call(null,map__45005__$1,new cljs.core.Keyword(null,"on-jsload","on-jsload",-395756602));
var reload_dependents = cljs.core.get.call(null,map__45005__$1,new cljs.core.Keyword(null,"reload-dependents","reload-dependents",-956865430));
var map__45006 = p__45004;
var map__45006__$1 = (((((!((map__45006 == null))))?(((((map__45006.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__45006.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__45006):map__45006);
var msg = map__45006__$1;
var files = cljs.core.get.call(null,map__45006__$1,new cljs.core.Keyword(null,"files","files",-472457450));
var figwheel_meta = cljs.core.get.call(null,map__45006__$1,new cljs.core.Keyword(null,"figwheel-meta","figwheel-meta",-225970237));
var recompile_dependents = cljs.core.get.call(null,map__45006__$1,new cljs.core.Keyword(null,"recompile-dependents","recompile-dependents",523804171));
if(cljs.core.empty_QMARK_.call(null,figwheel_meta)){
} else {
cljs.core.reset_BANG_.call(null,figwheel.client.file_reloading.figwheel_meta_pragmas,figwheel_meta);
}

var c__26800__auto__ = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_45160){
var state_val_45161 = (state_45160[(1)]);
if((state_val_45161 === (7))){
var inst_45020 = (state_45160[(7)]);
var inst_45022 = (state_45160[(8)]);
var inst_45023 = (state_45160[(9)]);
var inst_45021 = (state_45160[(10)]);
var inst_45028 = cljs.core._nth.call(null,inst_45021,inst_45023);
var inst_45029 = figwheel.client.file_reloading.eval_body.call(null,inst_45028,opts);
var inst_45030 = (inst_45023 + (1));
var tmp45162 = inst_45020;
var tmp45163 = inst_45022;
var tmp45164 = inst_45021;
var inst_45020__$1 = tmp45162;
var inst_45021__$1 = tmp45164;
var inst_45022__$1 = tmp45163;
var inst_45023__$1 = inst_45030;
var state_45160__$1 = (function (){var statearr_45165 = state_45160;
(statearr_45165[(7)] = inst_45020__$1);

(statearr_45165[(8)] = inst_45022__$1);

(statearr_45165[(9)] = inst_45023__$1);

(statearr_45165[(10)] = inst_45021__$1);

(statearr_45165[(11)] = inst_45029);

return statearr_45165;
})();
var statearr_45166_45249 = state_45160__$1;
(statearr_45166_45249[(2)] = null);

(statearr_45166_45249[(1)] = (5));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (20))){
var inst_45063 = (state_45160[(12)]);
var inst_45071 = figwheel.client.file_reloading.sort_files.call(null,inst_45063);
var state_45160__$1 = state_45160;
var statearr_45167_45250 = state_45160__$1;
(statearr_45167_45250[(2)] = inst_45071);

(statearr_45167_45250[(1)] = (21));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (27))){
var state_45160__$1 = state_45160;
var statearr_45168_45251 = state_45160__$1;
(statearr_45168_45251[(2)] = null);

(statearr_45168_45251[(1)] = (28));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (1))){
var inst_45012 = (state_45160[(13)]);
var inst_45009 = before_jsload.call(null,files);
var inst_45010 = figwheel.client.file_reloading.before_jsload_custom_event.call(null,files);
var inst_45011 = (function (){return (function (p1__45000_SHARP_){
return new cljs.core.Keyword(null,"eval-body","eval-body",-907279883).cljs$core$IFn$_invoke$arity$1(p1__45000_SHARP_);
});
})();
var inst_45012__$1 = cljs.core.filter.call(null,inst_45011,files);
var inst_45013 = cljs.core.not_empty.call(null,inst_45012__$1);
var state_45160__$1 = (function (){var statearr_45169 = state_45160;
(statearr_45169[(14)] = inst_45009);

(statearr_45169[(15)] = inst_45010);

(statearr_45169[(13)] = inst_45012__$1);

return statearr_45169;
})();
if(cljs.core.truth_(inst_45013)){
var statearr_45170_45252 = state_45160__$1;
(statearr_45170_45252[(1)] = (2));

} else {
var statearr_45171_45253 = state_45160__$1;
(statearr_45171_45253[(1)] = (3));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (24))){
var state_45160__$1 = state_45160;
var statearr_45172_45254 = state_45160__$1;
(statearr_45172_45254[(2)] = null);

(statearr_45172_45254[(1)] = (25));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (39))){
var inst_45113 = (state_45160[(16)]);
var state_45160__$1 = state_45160;
var statearr_45173_45255 = state_45160__$1;
(statearr_45173_45255[(2)] = inst_45113);

(statearr_45173_45255[(1)] = (40));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (46))){
var inst_45155 = (state_45160[(2)]);
var state_45160__$1 = state_45160;
var statearr_45174_45256 = state_45160__$1;
(statearr_45174_45256[(2)] = inst_45155);

(statearr_45174_45256[(1)] = (31));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (4))){
var inst_45057 = (state_45160[(2)]);
var inst_45058 = cljs.core.List.EMPTY;
var inst_45059 = cljs.core.reset_BANG_.call(null,figwheel.client.file_reloading.dependencies_loaded,inst_45058);
var inst_45060 = (function (){return (function (p1__45001_SHARP_){
var and__4115__auto__ = new cljs.core.Keyword(null,"namespace","namespace",-377510372).cljs$core$IFn$_invoke$arity$1(p1__45001_SHARP_);
if(cljs.core.truth_(and__4115__auto__)){
return ((cljs.core.not.call(null,new cljs.core.Keyword(null,"eval-body","eval-body",-907279883).cljs$core$IFn$_invoke$arity$1(p1__45001_SHARP_))) && (cljs.core.not.call(null,figwheel.client.file_reloading.figwheel_no_load_QMARK_.call(null,p1__45001_SHARP_))));
} else {
return and__4115__auto__;
}
});
})();
var inst_45061 = cljs.core.filter.call(null,inst_45060,files);
var inst_45062 = figwheel.client.file_reloading.get_figwheel_always.call(null);
var inst_45063 = cljs.core.concat.call(null,inst_45061,inst_45062);
var state_45160__$1 = (function (){var statearr_45175 = state_45160;
(statearr_45175[(17)] = inst_45059);

(statearr_45175[(18)] = inst_45057);

(statearr_45175[(12)] = inst_45063);

return statearr_45175;
})();
if(cljs.core.truth_(reload_dependents)){
var statearr_45176_45257 = state_45160__$1;
(statearr_45176_45257[(1)] = (16));

} else {
var statearr_45177_45258 = state_45160__$1;
(statearr_45177_45258[(1)] = (17));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (15))){
var inst_45047 = (state_45160[(2)]);
var state_45160__$1 = state_45160;
var statearr_45178_45259 = state_45160__$1;
(statearr_45178_45259[(2)] = inst_45047);

(statearr_45178_45259[(1)] = (12));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (21))){
var inst_45073 = (state_45160[(19)]);
var inst_45073__$1 = (state_45160[(2)]);
var inst_45074 = figwheel.client.file_reloading.load_all_js_files.call(null,inst_45073__$1);
var state_45160__$1 = (function (){var statearr_45179 = state_45160;
(statearr_45179[(19)] = inst_45073__$1);

return statearr_45179;
})();
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_45160__$1,(22),inst_45074);
} else {
if((state_val_45161 === (31))){
var inst_45158 = (state_45160[(2)]);
var state_45160__$1 = state_45160;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_45160__$1,inst_45158);
} else {
if((state_val_45161 === (32))){
var inst_45113 = (state_45160[(16)]);
var inst_45118 = inst_45113.cljs$lang$protocol_mask$partition0$;
var inst_45119 = (inst_45118 & (64));
var inst_45120 = inst_45113.cljs$core$ISeq$;
var inst_45121 = (cljs.core.PROTOCOL_SENTINEL === inst_45120);
var inst_45122 = ((inst_45119) || (inst_45121));
var state_45160__$1 = state_45160;
if(cljs.core.truth_(inst_45122)){
var statearr_45180_45260 = state_45160__$1;
(statearr_45180_45260[(1)] = (35));

} else {
var statearr_45181_45261 = state_45160__$1;
(statearr_45181_45261[(1)] = (36));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (40))){
var inst_45135 = (state_45160[(20)]);
var inst_45134 = (state_45160[(2)]);
var inst_45135__$1 = cljs.core.get.call(null,inst_45134,new cljs.core.Keyword(null,"figwheel-no-load","figwheel-no-load",-555840179));
var inst_45136 = cljs.core.get.call(null,inst_45134,new cljs.core.Keyword(null,"not-required","not-required",-950359114));
var inst_45137 = cljs.core.not_empty.call(null,inst_45135__$1);
var state_45160__$1 = (function (){var statearr_45182 = state_45160;
(statearr_45182[(20)] = inst_45135__$1);

(statearr_45182[(21)] = inst_45136);

return statearr_45182;
})();
if(cljs.core.truth_(inst_45137)){
var statearr_45183_45262 = state_45160__$1;
(statearr_45183_45262[(1)] = (41));

} else {
var statearr_45184_45263 = state_45160__$1;
(statearr_45184_45263[(1)] = (42));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (33))){
var state_45160__$1 = state_45160;
var statearr_45185_45264 = state_45160__$1;
(statearr_45185_45264[(2)] = false);

(statearr_45185_45264[(1)] = (34));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (13))){
var inst_45033 = (state_45160[(22)]);
var inst_45037 = cljs.core.chunk_first.call(null,inst_45033);
var inst_45038 = cljs.core.chunk_rest.call(null,inst_45033);
var inst_45039 = cljs.core.count.call(null,inst_45037);
var inst_45020 = inst_45038;
var inst_45021 = inst_45037;
var inst_45022 = inst_45039;
var inst_45023 = (0);
var state_45160__$1 = (function (){var statearr_45186 = state_45160;
(statearr_45186[(7)] = inst_45020);

(statearr_45186[(8)] = inst_45022);

(statearr_45186[(9)] = inst_45023);

(statearr_45186[(10)] = inst_45021);

return statearr_45186;
})();
var statearr_45187_45265 = state_45160__$1;
(statearr_45187_45265[(2)] = null);

(statearr_45187_45265[(1)] = (5));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (22))){
var inst_45081 = (state_45160[(23)]);
var inst_45076 = (state_45160[(24)]);
var inst_45073 = (state_45160[(19)]);
var inst_45077 = (state_45160[(25)]);
var inst_45076__$1 = (state_45160[(2)]);
var inst_45077__$1 = cljs.core.filter.call(null,new cljs.core.Keyword(null,"loaded-file","loaded-file",-168399375),inst_45076__$1);
var inst_45078 = (function (){var all_files = inst_45073;
var res_SINGLEQUOTE_ = inst_45076__$1;
var res = inst_45077__$1;
return (function (p1__45002_SHARP_){
return cljs.core.not.call(null,new cljs.core.Keyword(null,"loaded-file","loaded-file",-168399375).cljs$core$IFn$_invoke$arity$1(p1__45002_SHARP_));
});
})();
var inst_45079 = cljs.core.filter.call(null,inst_45078,inst_45076__$1);
var inst_45080 = cljs.core.deref.call(null,figwheel.client.file_reloading.dependencies_loaded);
var inst_45081__$1 = cljs.core.filter.call(null,new cljs.core.Keyword(null,"loaded-file","loaded-file",-168399375),inst_45080);
var inst_45082 = cljs.core.not_empty.call(null,inst_45081__$1);
var state_45160__$1 = (function (){var statearr_45188 = state_45160;
(statearr_45188[(26)] = inst_45079);

(statearr_45188[(23)] = inst_45081__$1);

(statearr_45188[(24)] = inst_45076__$1);

(statearr_45188[(25)] = inst_45077__$1);

return statearr_45188;
})();
if(cljs.core.truth_(inst_45082)){
var statearr_45189_45266 = state_45160__$1;
(statearr_45189_45266[(1)] = (23));

} else {
var statearr_45190_45267 = state_45160__$1;
(statearr_45190_45267[(1)] = (24));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (36))){
var state_45160__$1 = state_45160;
var statearr_45191_45268 = state_45160__$1;
(statearr_45191_45268[(2)] = false);

(statearr_45191_45268[(1)] = (37));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (41))){
var inst_45135 = (state_45160[(20)]);
var inst_45139 = cljs.core.comp.call(null,figwheel.client.file_reloading.name__GT_path,new cljs.core.Keyword(null,"namespace","namespace",-377510372));
var inst_45140 = cljs.core.map.call(null,inst_45139,inst_45135);
var inst_45141 = cljs.core.pr_str.call(null,inst_45140);
var inst_45142 = ["figwheel-no-load meta-data: ",inst_45141].join('');
var inst_45143 = figwheel.client.utils.log.call(null,inst_45142);
var state_45160__$1 = state_45160;
var statearr_45192_45269 = state_45160__$1;
(statearr_45192_45269[(2)] = inst_45143);

(statearr_45192_45269[(1)] = (43));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (43))){
var inst_45136 = (state_45160[(21)]);
var inst_45146 = (state_45160[(2)]);
var inst_45147 = cljs.core.not_empty.call(null,inst_45136);
var state_45160__$1 = (function (){var statearr_45193 = state_45160;
(statearr_45193[(27)] = inst_45146);

return statearr_45193;
})();
if(cljs.core.truth_(inst_45147)){
var statearr_45194_45270 = state_45160__$1;
(statearr_45194_45270[(1)] = (44));

} else {
var statearr_45195_45271 = state_45160__$1;
(statearr_45195_45271[(1)] = (45));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (29))){
var inst_45079 = (state_45160[(26)]);
var inst_45081 = (state_45160[(23)]);
var inst_45076 = (state_45160[(24)]);
var inst_45073 = (state_45160[(19)]);
var inst_45113 = (state_45160[(16)]);
var inst_45077 = (state_45160[(25)]);
var inst_45109 = figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"debug","debug",-1608172596),"Figwheel: NOT loading these files ");
var inst_45112 = (function (){var all_files = inst_45073;
var res_SINGLEQUOTE_ = inst_45076;
var res = inst_45077;
var files_not_loaded = inst_45079;
var dependencies_that_loaded = inst_45081;
return (function (p__45111){
var map__45196 = p__45111;
var map__45196__$1 = (((((!((map__45196 == null))))?(((((map__45196.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__45196.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__45196):map__45196);
var namespace = cljs.core.get.call(null,map__45196__$1,new cljs.core.Keyword(null,"namespace","namespace",-377510372));
var meta_data = cljs.core.get.call(null,cljs.core.deref.call(null,figwheel.client.file_reloading.figwheel_meta_pragmas),cljs.core.name.call(null,namespace));
if((meta_data == null)){
return new cljs.core.Keyword(null,"not-required","not-required",-950359114);
} else {
if(cljs.core.truth_(meta_data.call(null,new cljs.core.Keyword(null,"figwheel-no-load","figwheel-no-load",-555840179)))){
return new cljs.core.Keyword(null,"figwheel-no-load","figwheel-no-load",-555840179);
} else {
return new cljs.core.Keyword(null,"not-required","not-required",-950359114);

}
}
});
})();
var inst_45113__$1 = cljs.core.group_by.call(null,inst_45112,inst_45079);
var inst_45115 = (inst_45113__$1 == null);
var inst_45116 = cljs.core.not.call(null,inst_45115);
var state_45160__$1 = (function (){var statearr_45198 = state_45160;
(statearr_45198[(16)] = inst_45113__$1);

(statearr_45198[(28)] = inst_45109);

return statearr_45198;
})();
if(inst_45116){
var statearr_45199_45272 = state_45160__$1;
(statearr_45199_45272[(1)] = (32));

} else {
var statearr_45200_45273 = state_45160__$1;
(statearr_45200_45273[(1)] = (33));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (44))){
var inst_45136 = (state_45160[(21)]);
var inst_45149 = cljs.core.map.call(null,new cljs.core.Keyword(null,"file","file",-1269645878),inst_45136);
var inst_45150 = cljs.core.pr_str.call(null,inst_45149);
var inst_45151 = ["not required: ",inst_45150].join('');
var inst_45152 = figwheel.client.utils.log.call(null,inst_45151);
var state_45160__$1 = state_45160;
var statearr_45201_45274 = state_45160__$1;
(statearr_45201_45274[(2)] = inst_45152);

(statearr_45201_45274[(1)] = (46));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (6))){
var inst_45054 = (state_45160[(2)]);
var state_45160__$1 = state_45160;
var statearr_45202_45275 = state_45160__$1;
(statearr_45202_45275[(2)] = inst_45054);

(statearr_45202_45275[(1)] = (4));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (28))){
var inst_45079 = (state_45160[(26)]);
var inst_45106 = (state_45160[(2)]);
var inst_45107 = cljs.core.not_empty.call(null,inst_45079);
var state_45160__$1 = (function (){var statearr_45203 = state_45160;
(statearr_45203[(29)] = inst_45106);

return statearr_45203;
})();
if(cljs.core.truth_(inst_45107)){
var statearr_45204_45276 = state_45160__$1;
(statearr_45204_45276[(1)] = (29));

} else {
var statearr_45205_45277 = state_45160__$1;
(statearr_45205_45277[(1)] = (30));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (25))){
var inst_45077 = (state_45160[(25)]);
var inst_45093 = (state_45160[(2)]);
var inst_45094 = cljs.core.not_empty.call(null,inst_45077);
var state_45160__$1 = (function (){var statearr_45206 = state_45160;
(statearr_45206[(30)] = inst_45093);

return statearr_45206;
})();
if(cljs.core.truth_(inst_45094)){
var statearr_45207_45278 = state_45160__$1;
(statearr_45207_45278[(1)] = (26));

} else {
var statearr_45208_45279 = state_45160__$1;
(statearr_45208_45279[(1)] = (27));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (34))){
var inst_45129 = (state_45160[(2)]);
var state_45160__$1 = state_45160;
if(cljs.core.truth_(inst_45129)){
var statearr_45209_45280 = state_45160__$1;
(statearr_45209_45280[(1)] = (38));

} else {
var statearr_45210_45281 = state_45160__$1;
(statearr_45210_45281[(1)] = (39));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (17))){
var state_45160__$1 = state_45160;
var statearr_45211_45282 = state_45160__$1;
(statearr_45211_45282[(2)] = recompile_dependents);

(statearr_45211_45282[(1)] = (18));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (3))){
var state_45160__$1 = state_45160;
var statearr_45212_45283 = state_45160__$1;
(statearr_45212_45283[(2)] = null);

(statearr_45212_45283[(1)] = (4));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (12))){
var inst_45050 = (state_45160[(2)]);
var state_45160__$1 = state_45160;
var statearr_45213_45284 = state_45160__$1;
(statearr_45213_45284[(2)] = inst_45050);

(statearr_45213_45284[(1)] = (9));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (2))){
var inst_45012 = (state_45160[(13)]);
var inst_45019 = cljs.core.seq.call(null,inst_45012);
var inst_45020 = inst_45019;
var inst_45021 = null;
var inst_45022 = (0);
var inst_45023 = (0);
var state_45160__$1 = (function (){var statearr_45214 = state_45160;
(statearr_45214[(7)] = inst_45020);

(statearr_45214[(8)] = inst_45022);

(statearr_45214[(9)] = inst_45023);

(statearr_45214[(10)] = inst_45021);

return statearr_45214;
})();
var statearr_45215_45285 = state_45160__$1;
(statearr_45215_45285[(2)] = null);

(statearr_45215_45285[(1)] = (5));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (23))){
var inst_45079 = (state_45160[(26)]);
var inst_45081 = (state_45160[(23)]);
var inst_45076 = (state_45160[(24)]);
var inst_45073 = (state_45160[(19)]);
var inst_45077 = (state_45160[(25)]);
var inst_45084 = figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"debug","debug",-1608172596),"Figwheel: loaded these dependencies");
var inst_45086 = (function (){var all_files = inst_45073;
var res_SINGLEQUOTE_ = inst_45076;
var res = inst_45077;
var files_not_loaded = inst_45079;
var dependencies_that_loaded = inst_45081;
return (function (p__45085){
var map__45216 = p__45085;
var map__45216__$1 = (((((!((map__45216 == null))))?(((((map__45216.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__45216.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__45216):map__45216);
var request_url = cljs.core.get.call(null,map__45216__$1,new cljs.core.Keyword(null,"request-url","request-url",2100346596));
return clojure.string.replace.call(null,request_url,goog.basePath,"");
});
})();
var inst_45087 = cljs.core.reverse.call(null,inst_45081);
var inst_45088 = cljs.core.map.call(null,inst_45086,inst_45087);
var inst_45089 = cljs.core.pr_str.call(null,inst_45088);
var inst_45090 = figwheel.client.utils.log.call(null,inst_45089);
var state_45160__$1 = (function (){var statearr_45218 = state_45160;
(statearr_45218[(31)] = inst_45084);

return statearr_45218;
})();
var statearr_45219_45286 = state_45160__$1;
(statearr_45219_45286[(2)] = inst_45090);

(statearr_45219_45286[(1)] = (25));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (35))){
var state_45160__$1 = state_45160;
var statearr_45220_45287 = state_45160__$1;
(statearr_45220_45287[(2)] = true);

(statearr_45220_45287[(1)] = (37));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (19))){
var inst_45063 = (state_45160[(12)]);
var inst_45069 = figwheel.client.file_reloading.expand_files.call(null,inst_45063);
var state_45160__$1 = state_45160;
var statearr_45221_45288 = state_45160__$1;
(statearr_45221_45288[(2)] = inst_45069);

(statearr_45221_45288[(1)] = (21));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (11))){
var state_45160__$1 = state_45160;
var statearr_45222_45289 = state_45160__$1;
(statearr_45222_45289[(2)] = null);

(statearr_45222_45289[(1)] = (12));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (9))){
var inst_45052 = (state_45160[(2)]);
var state_45160__$1 = state_45160;
var statearr_45223_45290 = state_45160__$1;
(statearr_45223_45290[(2)] = inst_45052);

(statearr_45223_45290[(1)] = (6));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (5))){
var inst_45022 = (state_45160[(8)]);
var inst_45023 = (state_45160[(9)]);
var inst_45025 = (inst_45023 < inst_45022);
var inst_45026 = inst_45025;
var state_45160__$1 = state_45160;
if(cljs.core.truth_(inst_45026)){
var statearr_45224_45291 = state_45160__$1;
(statearr_45224_45291[(1)] = (7));

} else {
var statearr_45225_45292 = state_45160__$1;
(statearr_45225_45292[(1)] = (8));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (14))){
var inst_45033 = (state_45160[(22)]);
var inst_45042 = cljs.core.first.call(null,inst_45033);
var inst_45043 = figwheel.client.file_reloading.eval_body.call(null,inst_45042,opts);
var inst_45044 = cljs.core.next.call(null,inst_45033);
var inst_45020 = inst_45044;
var inst_45021 = null;
var inst_45022 = (0);
var inst_45023 = (0);
var state_45160__$1 = (function (){var statearr_45226 = state_45160;
(statearr_45226[(32)] = inst_45043);

(statearr_45226[(7)] = inst_45020);

(statearr_45226[(8)] = inst_45022);

(statearr_45226[(9)] = inst_45023);

(statearr_45226[(10)] = inst_45021);

return statearr_45226;
})();
var statearr_45227_45293 = state_45160__$1;
(statearr_45227_45293[(2)] = null);

(statearr_45227_45293[(1)] = (5));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (45))){
var state_45160__$1 = state_45160;
var statearr_45228_45294 = state_45160__$1;
(statearr_45228_45294[(2)] = null);

(statearr_45228_45294[(1)] = (46));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (26))){
var inst_45079 = (state_45160[(26)]);
var inst_45081 = (state_45160[(23)]);
var inst_45076 = (state_45160[(24)]);
var inst_45073 = (state_45160[(19)]);
var inst_45077 = (state_45160[(25)]);
var inst_45096 = figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"debug","debug",-1608172596),"Figwheel: loaded these files");
var inst_45098 = (function (){var all_files = inst_45073;
var res_SINGLEQUOTE_ = inst_45076;
var res = inst_45077;
var files_not_loaded = inst_45079;
var dependencies_that_loaded = inst_45081;
return (function (p__45097){
var map__45229 = p__45097;
var map__45229__$1 = (((((!((map__45229 == null))))?(((((map__45229.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__45229.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__45229):map__45229);
var namespace = cljs.core.get.call(null,map__45229__$1,new cljs.core.Keyword(null,"namespace","namespace",-377510372));
var file = cljs.core.get.call(null,map__45229__$1,new cljs.core.Keyword(null,"file","file",-1269645878));
if(cljs.core.truth_(namespace)){
return figwheel.client.file_reloading.name__GT_path.call(null,cljs.core.name.call(null,namespace));
} else {
return file;
}
});
})();
var inst_45099 = cljs.core.map.call(null,inst_45098,inst_45077);
var inst_45100 = cljs.core.pr_str.call(null,inst_45099);
var inst_45101 = figwheel.client.utils.log.call(null,inst_45100);
var inst_45102 = (function (){var all_files = inst_45073;
var res_SINGLEQUOTE_ = inst_45076;
var res = inst_45077;
var files_not_loaded = inst_45079;
var dependencies_that_loaded = inst_45081;
return (function (){
figwheel.client.file_reloading.on_jsload_custom_event.call(null,res);

return cljs.core.apply.call(null,on_jsload,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [res], null));
});
})();
var inst_45103 = setTimeout(inst_45102,(10));
var state_45160__$1 = (function (){var statearr_45231 = state_45160;
(statearr_45231[(33)] = inst_45101);

(statearr_45231[(34)] = inst_45096);

return statearr_45231;
})();
var statearr_45232_45295 = state_45160__$1;
(statearr_45232_45295[(2)] = inst_45103);

(statearr_45232_45295[(1)] = (28));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (16))){
var state_45160__$1 = state_45160;
var statearr_45233_45296 = state_45160__$1;
(statearr_45233_45296[(2)] = reload_dependents);

(statearr_45233_45296[(1)] = (18));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (38))){
var inst_45113 = (state_45160[(16)]);
var inst_45131 = cljs.core.apply.call(null,cljs.core.hash_map,inst_45113);
var state_45160__$1 = state_45160;
var statearr_45234_45297 = state_45160__$1;
(statearr_45234_45297[(2)] = inst_45131);

(statearr_45234_45297[(1)] = (40));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (30))){
var state_45160__$1 = state_45160;
var statearr_45235_45298 = state_45160__$1;
(statearr_45235_45298[(2)] = null);

(statearr_45235_45298[(1)] = (31));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (10))){
var inst_45033 = (state_45160[(22)]);
var inst_45035 = cljs.core.chunked_seq_QMARK_.call(null,inst_45033);
var state_45160__$1 = state_45160;
if(inst_45035){
var statearr_45236_45299 = state_45160__$1;
(statearr_45236_45299[(1)] = (13));

} else {
var statearr_45237_45300 = state_45160__$1;
(statearr_45237_45300[(1)] = (14));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (18))){
var inst_45067 = (state_45160[(2)]);
var state_45160__$1 = state_45160;
if(cljs.core.truth_(inst_45067)){
var statearr_45238_45301 = state_45160__$1;
(statearr_45238_45301[(1)] = (19));

} else {
var statearr_45239_45302 = state_45160__$1;
(statearr_45239_45302[(1)] = (20));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (42))){
var state_45160__$1 = state_45160;
var statearr_45240_45303 = state_45160__$1;
(statearr_45240_45303[(2)] = null);

(statearr_45240_45303[(1)] = (43));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (37))){
var inst_45126 = (state_45160[(2)]);
var state_45160__$1 = state_45160;
var statearr_45241_45304 = state_45160__$1;
(statearr_45241_45304[(2)] = inst_45126);

(statearr_45241_45304[(1)] = (34));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_45161 === (8))){
var inst_45033 = (state_45160[(22)]);
var inst_45020 = (state_45160[(7)]);
var inst_45033__$1 = cljs.core.seq.call(null,inst_45020);
var state_45160__$1 = (function (){var statearr_45242 = state_45160;
(statearr_45242[(22)] = inst_45033__$1);

return statearr_45242;
})();
if(inst_45033__$1){
var statearr_45243_45305 = state_45160__$1;
(statearr_45243_45305[(1)] = (10));

} else {
var statearr_45244_45306 = state_45160__$1;
(statearr_45244_45306[(1)] = (11));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
return null;
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
}
});
return (function() {
var figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto__ = null;
var figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto____0 = (function (){
var statearr_45245 = [null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_45245[(0)] = figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto__);

(statearr_45245[(1)] = (1));

return statearr_45245;
});
var figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto____1 = (function (state_45160){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_45160);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e45246){if((e45246 instanceof Object)){
var ex__26709__auto__ = e45246;
var statearr_45247_45307 = state_45160;
(statearr_45247_45307[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_45160);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e45246;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__45308 = state_45160;
state_45160 = G__45308;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto__ = function(state_45160){
switch(arguments.length){
case 0:
return figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto____0.call(this);
case 1:
return figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto____1.call(this,state_45160);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto____0;
figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto____1;
return figwheel$client$file_reloading$reload_js_files_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_45248 = f__26801__auto__.call(null);
(statearr_45248[(6)] = c__26800__auto__);

return statearr_45248;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));

return c__26800__auto__;
});
figwheel.client.file_reloading.current_links = (function figwheel$client$file_reloading$current_links(){
return Array.prototype.slice.call(document.getElementsByTagName("link"));
});
figwheel.client.file_reloading.truncate_url = (function figwheel$client$file_reloading$truncate_url(url){
return clojure.string.replace_first.call(null,clojure.string.replace_first.call(null,clojure.string.replace_first.call(null,clojure.string.replace_first.call(null,cljs.core.first.call(null,clojure.string.split.call(null,url,/\?/)),[cljs.core.str.cljs$core$IFn$_invoke$arity$1(location.protocol),"//"].join(''),""),".*://",""),/^\/\//,""),/[^\\/]*/,"");
});
figwheel.client.file_reloading.matches_file_QMARK_ = (function figwheel$client$file_reloading$matches_file_QMARK_(p__45311,link){
var map__45312 = p__45311;
var map__45312__$1 = (((((!((map__45312 == null))))?(((((map__45312.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__45312.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__45312):map__45312);
var file = cljs.core.get.call(null,map__45312__$1,new cljs.core.Keyword(null,"file","file",-1269645878));
var temp__5720__auto__ = link.href;
if(cljs.core.truth_(temp__5720__auto__)){
var link_href = temp__5720__auto__;
var match = clojure.string.join.call(null,"/",cljs.core.take_while.call(null,cljs.core.identity,cljs.core.map.call(null,(function (p1__45309_SHARP_,p2__45310_SHARP_){
if(cljs.core._EQ_.call(null,p1__45309_SHARP_,p2__45310_SHARP_)){
return p1__45309_SHARP_;
} else {
return false;
}
}),cljs.core.reverse.call(null,clojure.string.split.call(null,file,"/")),cljs.core.reverse.call(null,clojure.string.split.call(null,figwheel.client.file_reloading.truncate_url.call(null,link_href),"/")))));
var match_length = ((match).length);
var file_name_length = cljs.core.count.call(null,cljs.core.last.call(null,clojure.string.split.call(null,file,"/")));
if((match_length >= file_name_length)){
return new cljs.core.PersistentArrayMap(null, 4, [new cljs.core.Keyword(null,"link","link",-1769163468),link,new cljs.core.Keyword(null,"link-href","link-href",-250644450),link_href,new cljs.core.Keyword(null,"match-length","match-length",1101537310),match_length,new cljs.core.Keyword(null,"current-url-length","current-url-length",380404083),((figwheel.client.file_reloading.truncate_url.call(null,link_href)).length)], null);
} else {
return null;
}
} else {
return null;
}
});
figwheel.client.file_reloading.get_correct_link = (function figwheel$client$file_reloading$get_correct_link(f_data){
var temp__5720__auto__ = cljs.core.first.call(null,cljs.core.sort_by.call(null,(function (p__45315){
var map__45316 = p__45315;
var map__45316__$1 = (((((!((map__45316 == null))))?(((((map__45316.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__45316.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__45316):map__45316);
var match_length = cljs.core.get.call(null,map__45316__$1,new cljs.core.Keyword(null,"match-length","match-length",1101537310));
var current_url_length = cljs.core.get.call(null,map__45316__$1,new cljs.core.Keyword(null,"current-url-length","current-url-length",380404083));
return (current_url_length - match_length);
}),cljs.core.keep.call(null,(function (p1__45314_SHARP_){
return figwheel.client.file_reloading.matches_file_QMARK_.call(null,f_data,p1__45314_SHARP_);
}),figwheel.client.file_reloading.current_links.call(null))));
if(cljs.core.truth_(temp__5720__auto__)){
var res = temp__5720__auto__;
return new cljs.core.Keyword(null,"link","link",-1769163468).cljs$core$IFn$_invoke$arity$1(res);
} else {
return null;
}
});
figwheel.client.file_reloading.clone_link = (function figwheel$client$file_reloading$clone_link(link,url){
var clone = document.createElement("link");
(clone.rel = "stylesheet");

(clone.media = link.media);

(clone.disabled = link.disabled);

(clone.href = figwheel.client.file_reloading.add_cache_buster.call(null,url));

return clone;
});
figwheel.client.file_reloading.create_link = (function figwheel$client$file_reloading$create_link(url){
var link = document.createElement("link");
(link.rel = "stylesheet");

(link.href = figwheel.client.file_reloading.add_cache_buster.call(null,url));

return link;
});
figwheel.client.file_reloading.distinctify = (function figwheel$client$file_reloading$distinctify(key,seqq){
return cljs.core.vals.call(null,cljs.core.reduce.call(null,(function (p1__45318_SHARP_,p2__45319_SHARP_){
return cljs.core.assoc.call(null,p1__45318_SHARP_,cljs.core.get.call(null,p2__45319_SHARP_,key),p2__45319_SHARP_);
}),cljs.core.PersistentArrayMap.EMPTY,seqq));
});
figwheel.client.file_reloading.add_link_to_document = (function figwheel$client$file_reloading$add_link_to_document(orig_link,klone,finished_fn){
var parent = orig_link.parentNode;
if(cljs.core._EQ_.call(null,orig_link,parent.lastChild)){
parent.appendChild(klone);
} else {
parent.insertBefore(klone,orig_link.nextSibling);
}

return setTimeout((function (){
parent.removeChild(orig_link);

return finished_fn.call(null);
}),(300));
});
if((typeof figwheel !== 'undefined') && (typeof figwheel.client !== 'undefined') && (typeof figwheel.client.file_reloading !== 'undefined') && (typeof figwheel.client.file_reloading.reload_css_deferred_chain !== 'undefined')){
} else {
figwheel.client.file_reloading.reload_css_deferred_chain = cljs.core.atom.call(null,goog.async.Deferred.succeed());
}
figwheel.client.file_reloading.reload_css_file = (function figwheel$client$file_reloading$reload_css_file(f_data,fin){
var temp__5718__auto__ = figwheel.client.file_reloading.get_correct_link.call(null,f_data);
if(cljs.core.truth_(temp__5718__auto__)){
var link = temp__5718__auto__;
return figwheel.client.file_reloading.add_link_to_document.call(null,link,figwheel.client.file_reloading.clone_link.call(null,link,link.href),(function (){
return fin.call(null,cljs.core.assoc.call(null,f_data,new cljs.core.Keyword(null,"loaded","loaded",-1246482293),true));
}));
} else {
return fin.call(null,f_data);
}
});
figwheel.client.file_reloading.reload_css_files_STAR_ = (function figwheel$client$file_reloading$reload_css_files_STAR_(deferred,f_datas,on_cssload){
return figwheel.client.utils.liftContD.call(null,figwheel.client.utils.mapConcatD.call(null,deferred,figwheel.client.file_reloading.reload_css_file,f_datas),(function (f_datas_SINGLEQUOTE_,fin){
var loaded_f_datas_45320 = cljs.core.filter.call(null,new cljs.core.Keyword(null,"loaded","loaded",-1246482293),f_datas_SINGLEQUOTE_);
figwheel.client.file_reloading.on_cssload_custom_event.call(null,loaded_f_datas_45320);

if(cljs.core.fn_QMARK_.call(null,on_cssload)){
on_cssload.call(null,loaded_f_datas_45320);
} else {
}

return fin.call(null);
}));
});
figwheel.client.file_reloading.reload_css_files = (function figwheel$client$file_reloading$reload_css_files(p__45321,p__45322){
var map__45323 = p__45321;
var map__45323__$1 = (((((!((map__45323 == null))))?(((((map__45323.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__45323.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__45323):map__45323);
var on_cssload = cljs.core.get.call(null,map__45323__$1,new cljs.core.Keyword(null,"on-cssload","on-cssload",1825432318));
var map__45324 = p__45322;
var map__45324__$1 = (((((!((map__45324 == null))))?(((((map__45324.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__45324.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__45324):map__45324);
var files_msg = map__45324__$1;
var files = cljs.core.get.call(null,map__45324__$1,new cljs.core.Keyword(null,"files","files",-472457450));
if(figwheel.client.utils.html_env_QMARK_.call(null)){
var temp__5720__auto__ = cljs.core.not_empty.call(null,figwheel.client.file_reloading.distinctify.call(null,new cljs.core.Keyword(null,"file","file",-1269645878),files));
if(cljs.core.truth_(temp__5720__auto__)){
var f_datas = temp__5720__auto__;
return cljs.core.swap_BANG_.call(null,figwheel.client.file_reloading.reload_css_deferred_chain,figwheel.client.file_reloading.reload_css_files_STAR_,f_datas,on_cssload);
} else {
return null;
}
} else {
return null;
}
});

//# sourceMappingURL=file_reloading.js.map?rel=1654487552377
