// Compiled by ClojureScript 1.10.773 {}
goog.provide('cljs.repl');
goog.require('cljs.core');
goog.require('cljs.spec.alpha');
goog.require('goog.string');
goog.require('goog.string.format');
cljs.repl.print_doc = (function cljs$repl$print_doc(p__46191){
var map__46192 = p__46191;
var map__46192__$1 = (((((!((map__46192 == null))))?(((((map__46192.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46192.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46192):map__46192);
var m = map__46192__$1;
var n = cljs.core.get.call(null,map__46192__$1,new cljs.core.Keyword(null,"ns","ns",441598760));
var nm = cljs.core.get.call(null,map__46192__$1,new cljs.core.Keyword(null,"name","name",1843675177));
cljs.core.println.call(null,"-------------------------");

cljs.core.println.call(null,(function (){var or__4126__auto__ = new cljs.core.Keyword(null,"spec","spec",347520401).cljs$core$IFn$_invoke$arity$1(m);
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return [(function (){var temp__5720__auto__ = new cljs.core.Keyword(null,"ns","ns",441598760).cljs$core$IFn$_invoke$arity$1(m);
if(cljs.core.truth_(temp__5720__auto__)){
var ns = temp__5720__auto__;
return [cljs.core.str.cljs$core$IFn$_invoke$arity$1(ns),"/"].join('');
} else {
return null;
}
})(),cljs.core.str.cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"name","name",1843675177).cljs$core$IFn$_invoke$arity$1(m))].join('');
}
})());

if(cljs.core.truth_(new cljs.core.Keyword(null,"protocol","protocol",652470118).cljs$core$IFn$_invoke$arity$1(m))){
cljs.core.println.call(null,"Protocol");
} else {
}

if(cljs.core.truth_(new cljs.core.Keyword(null,"forms","forms",2045992350).cljs$core$IFn$_invoke$arity$1(m))){
var seq__46194_46226 = cljs.core.seq.call(null,new cljs.core.Keyword(null,"forms","forms",2045992350).cljs$core$IFn$_invoke$arity$1(m));
var chunk__46195_46227 = null;
var count__46196_46228 = (0);
var i__46197_46229 = (0);
while(true){
if((i__46197_46229 < count__46196_46228)){
var f_46230 = cljs.core._nth.call(null,chunk__46195_46227,i__46197_46229);
cljs.core.println.call(null,"  ",f_46230);


var G__46231 = seq__46194_46226;
var G__46232 = chunk__46195_46227;
var G__46233 = count__46196_46228;
var G__46234 = (i__46197_46229 + (1));
seq__46194_46226 = G__46231;
chunk__46195_46227 = G__46232;
count__46196_46228 = G__46233;
i__46197_46229 = G__46234;
continue;
} else {
var temp__5720__auto___46235 = cljs.core.seq.call(null,seq__46194_46226);
if(temp__5720__auto___46235){
var seq__46194_46236__$1 = temp__5720__auto___46235;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__46194_46236__$1)){
var c__4556__auto___46237 = cljs.core.chunk_first.call(null,seq__46194_46236__$1);
var G__46238 = cljs.core.chunk_rest.call(null,seq__46194_46236__$1);
var G__46239 = c__4556__auto___46237;
var G__46240 = cljs.core.count.call(null,c__4556__auto___46237);
var G__46241 = (0);
seq__46194_46226 = G__46238;
chunk__46195_46227 = G__46239;
count__46196_46228 = G__46240;
i__46197_46229 = G__46241;
continue;
} else {
var f_46242 = cljs.core.first.call(null,seq__46194_46236__$1);
cljs.core.println.call(null,"  ",f_46242);


var G__46243 = cljs.core.next.call(null,seq__46194_46236__$1);
var G__46244 = null;
var G__46245 = (0);
var G__46246 = (0);
seq__46194_46226 = G__46243;
chunk__46195_46227 = G__46244;
count__46196_46228 = G__46245;
i__46197_46229 = G__46246;
continue;
}
} else {
}
}
break;
}
} else {
if(cljs.core.truth_(new cljs.core.Keyword(null,"arglists","arglists",1661989754).cljs$core$IFn$_invoke$arity$1(m))){
var arglists_46247 = new cljs.core.Keyword(null,"arglists","arglists",1661989754).cljs$core$IFn$_invoke$arity$1(m);
if(cljs.core.truth_((function (){var or__4126__auto__ = new cljs.core.Keyword(null,"macro","macro",-867863404).cljs$core$IFn$_invoke$arity$1(m);
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return new cljs.core.Keyword(null,"repl-special-function","repl-special-function",1262603725).cljs$core$IFn$_invoke$arity$1(m);
}
})())){
cljs.core.prn.call(null,arglists_46247);
} else {
cljs.core.prn.call(null,((cljs.core._EQ_.call(null,new cljs.core.Symbol(null,"quote","quote",1377916282,null),cljs.core.first.call(null,arglists_46247)))?cljs.core.second.call(null,arglists_46247):arglists_46247));
}
} else {
}
}

if(cljs.core.truth_(new cljs.core.Keyword(null,"special-form","special-form",-1326536374).cljs$core$IFn$_invoke$arity$1(m))){
cljs.core.println.call(null,"Special Form");

cljs.core.println.call(null," ",new cljs.core.Keyword(null,"doc","doc",1913296891).cljs$core$IFn$_invoke$arity$1(m));

if(cljs.core.contains_QMARK_.call(null,m,new cljs.core.Keyword(null,"url","url",276297046))){
if(cljs.core.truth_(new cljs.core.Keyword(null,"url","url",276297046).cljs$core$IFn$_invoke$arity$1(m))){
return cljs.core.println.call(null,["\n  Please see http://clojure.org/",cljs.core.str.cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"url","url",276297046).cljs$core$IFn$_invoke$arity$1(m))].join(''));
} else {
return null;
}
} else {
return cljs.core.println.call(null,["\n  Please see http://clojure.org/special_forms#",cljs.core.str.cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"name","name",1843675177).cljs$core$IFn$_invoke$arity$1(m))].join(''));
}
} else {
if(cljs.core.truth_(new cljs.core.Keyword(null,"macro","macro",-867863404).cljs$core$IFn$_invoke$arity$1(m))){
cljs.core.println.call(null,"Macro");
} else {
}

if(cljs.core.truth_(new cljs.core.Keyword(null,"spec","spec",347520401).cljs$core$IFn$_invoke$arity$1(m))){
cljs.core.println.call(null,"Spec");
} else {
}

if(cljs.core.truth_(new cljs.core.Keyword(null,"repl-special-function","repl-special-function",1262603725).cljs$core$IFn$_invoke$arity$1(m))){
cljs.core.println.call(null,"REPL Special Function");
} else {
}

cljs.core.println.call(null," ",new cljs.core.Keyword(null,"doc","doc",1913296891).cljs$core$IFn$_invoke$arity$1(m));

if(cljs.core.truth_(new cljs.core.Keyword(null,"protocol","protocol",652470118).cljs$core$IFn$_invoke$arity$1(m))){
var seq__46198_46248 = cljs.core.seq.call(null,new cljs.core.Keyword(null,"methods","methods",453930866).cljs$core$IFn$_invoke$arity$1(m));
var chunk__46199_46249 = null;
var count__46200_46250 = (0);
var i__46201_46251 = (0);
while(true){
if((i__46201_46251 < count__46200_46250)){
var vec__46212_46252 = cljs.core._nth.call(null,chunk__46199_46249,i__46201_46251);
var name_46253 = cljs.core.nth.call(null,vec__46212_46252,(0),null);
var map__46215_46254 = cljs.core.nth.call(null,vec__46212_46252,(1),null);
var map__46215_46255__$1 = (((((!((map__46215_46254 == null))))?(((((map__46215_46254.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46215_46254.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46215_46254):map__46215_46254);
var doc_46256 = cljs.core.get.call(null,map__46215_46255__$1,new cljs.core.Keyword(null,"doc","doc",1913296891));
var arglists_46257 = cljs.core.get.call(null,map__46215_46255__$1,new cljs.core.Keyword(null,"arglists","arglists",1661989754));
cljs.core.println.call(null);

cljs.core.println.call(null," ",name_46253);

cljs.core.println.call(null," ",arglists_46257);

if(cljs.core.truth_(doc_46256)){
cljs.core.println.call(null," ",doc_46256);
} else {
}


var G__46258 = seq__46198_46248;
var G__46259 = chunk__46199_46249;
var G__46260 = count__46200_46250;
var G__46261 = (i__46201_46251 + (1));
seq__46198_46248 = G__46258;
chunk__46199_46249 = G__46259;
count__46200_46250 = G__46260;
i__46201_46251 = G__46261;
continue;
} else {
var temp__5720__auto___46262 = cljs.core.seq.call(null,seq__46198_46248);
if(temp__5720__auto___46262){
var seq__46198_46263__$1 = temp__5720__auto___46262;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__46198_46263__$1)){
var c__4556__auto___46264 = cljs.core.chunk_first.call(null,seq__46198_46263__$1);
var G__46265 = cljs.core.chunk_rest.call(null,seq__46198_46263__$1);
var G__46266 = c__4556__auto___46264;
var G__46267 = cljs.core.count.call(null,c__4556__auto___46264);
var G__46268 = (0);
seq__46198_46248 = G__46265;
chunk__46199_46249 = G__46266;
count__46200_46250 = G__46267;
i__46201_46251 = G__46268;
continue;
} else {
var vec__46217_46269 = cljs.core.first.call(null,seq__46198_46263__$1);
var name_46270 = cljs.core.nth.call(null,vec__46217_46269,(0),null);
var map__46220_46271 = cljs.core.nth.call(null,vec__46217_46269,(1),null);
var map__46220_46272__$1 = (((((!((map__46220_46271 == null))))?(((((map__46220_46271.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46220_46271.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46220_46271):map__46220_46271);
var doc_46273 = cljs.core.get.call(null,map__46220_46272__$1,new cljs.core.Keyword(null,"doc","doc",1913296891));
var arglists_46274 = cljs.core.get.call(null,map__46220_46272__$1,new cljs.core.Keyword(null,"arglists","arglists",1661989754));
cljs.core.println.call(null);

cljs.core.println.call(null," ",name_46270);

cljs.core.println.call(null," ",arglists_46274);

if(cljs.core.truth_(doc_46273)){
cljs.core.println.call(null," ",doc_46273);
} else {
}


var G__46275 = cljs.core.next.call(null,seq__46198_46263__$1);
var G__46276 = null;
var G__46277 = (0);
var G__46278 = (0);
seq__46198_46248 = G__46275;
chunk__46199_46249 = G__46276;
count__46200_46250 = G__46277;
i__46201_46251 = G__46278;
continue;
}
} else {
}
}
break;
}
} else {
}

if(cljs.core.truth_(n)){
var temp__5720__auto__ = cljs.spec.alpha.get_spec.call(null,cljs.core.symbol.call(null,cljs.core.str.cljs$core$IFn$_invoke$arity$1(cljs.core.ns_name.call(null,n)),cljs.core.name.call(null,nm)));
if(cljs.core.truth_(temp__5720__auto__)){
var fnspec = temp__5720__auto__;
cljs.core.print.call(null,"Spec");

var seq__46222 = cljs.core.seq.call(null,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"args","args",1315556576),new cljs.core.Keyword(null,"ret","ret",-468222814),new cljs.core.Keyword(null,"fn","fn",-1175266204)], null));
var chunk__46223 = null;
var count__46224 = (0);
var i__46225 = (0);
while(true){
if((i__46225 < count__46224)){
var role = cljs.core._nth.call(null,chunk__46223,i__46225);
var temp__5720__auto___46279__$1 = cljs.core.get.call(null,fnspec,role);
if(cljs.core.truth_(temp__5720__auto___46279__$1)){
var spec_46280 = temp__5720__auto___46279__$1;
cljs.core.print.call(null,["\n ",cljs.core.name.call(null,role),":"].join(''),cljs.spec.alpha.describe.call(null,spec_46280));
} else {
}


var G__46281 = seq__46222;
var G__46282 = chunk__46223;
var G__46283 = count__46224;
var G__46284 = (i__46225 + (1));
seq__46222 = G__46281;
chunk__46223 = G__46282;
count__46224 = G__46283;
i__46225 = G__46284;
continue;
} else {
var temp__5720__auto____$1 = cljs.core.seq.call(null,seq__46222);
if(temp__5720__auto____$1){
var seq__46222__$1 = temp__5720__auto____$1;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__46222__$1)){
var c__4556__auto__ = cljs.core.chunk_first.call(null,seq__46222__$1);
var G__46285 = cljs.core.chunk_rest.call(null,seq__46222__$1);
var G__46286 = c__4556__auto__;
var G__46287 = cljs.core.count.call(null,c__4556__auto__);
var G__46288 = (0);
seq__46222 = G__46285;
chunk__46223 = G__46286;
count__46224 = G__46287;
i__46225 = G__46288;
continue;
} else {
var role = cljs.core.first.call(null,seq__46222__$1);
var temp__5720__auto___46289__$2 = cljs.core.get.call(null,fnspec,role);
if(cljs.core.truth_(temp__5720__auto___46289__$2)){
var spec_46290 = temp__5720__auto___46289__$2;
cljs.core.print.call(null,["\n ",cljs.core.name.call(null,role),":"].join(''),cljs.spec.alpha.describe.call(null,spec_46290));
} else {
}


var G__46291 = cljs.core.next.call(null,seq__46222__$1);
var G__46292 = null;
var G__46293 = (0);
var G__46294 = (0);
seq__46222 = G__46291;
chunk__46223 = G__46292;
count__46224 = G__46293;
i__46225 = G__46294;
continue;
}
} else {
return null;
}
}
break;
}
} else {
return null;
}
} else {
return null;
}
}
});
/**
 * Constructs a data representation for a Error with keys:
 *  :cause - root cause message
 *  :phase - error phase
 *  :via - cause chain, with cause keys:
 *           :type - exception class symbol
 *           :message - exception message
 *           :data - ex-data
 *           :at - top stack element
 *  :trace - root cause stack elements
 */
cljs.repl.Error__GT_map = (function cljs$repl$Error__GT_map(o){
var base = (function (t){
return cljs.core.merge.call(null,new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"type","type",1174270348),(((t instanceof cljs.core.ExceptionInfo))?new cljs.core.Symbol(null,"ExceptionInfo","ExceptionInfo",294935087,null):(((t instanceof Error))?cljs.core.symbol.call(null,"js",t.name):null
))], null),(function (){var temp__5720__auto__ = cljs.core.ex_message.call(null,t);
if(cljs.core.truth_(temp__5720__auto__)){
var msg = temp__5720__auto__;
return new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"message","message",-406056002),msg], null);
} else {
return null;
}
})(),(function (){var temp__5720__auto__ = cljs.core.ex_data.call(null,t);
if(cljs.core.truth_(temp__5720__auto__)){
var ed = temp__5720__auto__;
return new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"data","data",-232669377),ed], null);
} else {
return null;
}
})());
});
var via = (function (){var via = cljs.core.PersistentVector.EMPTY;
var t = o;
while(true){
if(cljs.core.truth_(t)){
var G__46295 = cljs.core.conj.call(null,via,t);
var G__46296 = cljs.core.ex_cause.call(null,t);
via = G__46295;
t = G__46296;
continue;
} else {
return via;
}
break;
}
})();
var root = cljs.core.peek.call(null,via);
return cljs.core.merge.call(null,new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"via","via",-1904457336),cljs.core.vec.call(null,cljs.core.map.call(null,base,via)),new cljs.core.Keyword(null,"trace","trace",-1082747415),null], null),(function (){var temp__5720__auto__ = cljs.core.ex_message.call(null,root);
if(cljs.core.truth_(temp__5720__auto__)){
var root_msg = temp__5720__auto__;
return new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"cause","cause",231901252),root_msg], null);
} else {
return null;
}
})(),(function (){var temp__5720__auto__ = cljs.core.ex_data.call(null,root);
if(cljs.core.truth_(temp__5720__auto__)){
var data = temp__5720__auto__;
return new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"data","data",-232669377),data], null);
} else {
return null;
}
})(),(function (){var temp__5720__auto__ = new cljs.core.Keyword("clojure.error","phase","clojure.error/phase",275140358).cljs$core$IFn$_invoke$arity$1(cljs.core.ex_data.call(null,o));
if(cljs.core.truth_(temp__5720__auto__)){
var phase = temp__5720__auto__;
return new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"phase","phase",575722892),phase], null);
} else {
return null;
}
})());
});
/**
 * Returns an analysis of the phase, error, cause, and location of an error that occurred
 *   based on Throwable data, as returned by Throwable->map. All attributes other than phase
 *   are optional:
 *  :clojure.error/phase - keyword phase indicator, one of:
 *    :read-source :compile-syntax-check :compilation :macro-syntax-check :macroexpansion
 *    :execution :read-eval-result :print-eval-result
 *  :clojure.error/source - file name (no path)
 *  :clojure.error/line - integer line number
 *  :clojure.error/column - integer column number
 *  :clojure.error/symbol - symbol being expanded/compiled/invoked
 *  :clojure.error/class - cause exception class symbol
 *  :clojure.error/cause - cause exception message
 *  :clojure.error/spec - explain-data for spec error
 */
cljs.repl.ex_triage = (function cljs$repl$ex_triage(datafied_throwable){
var map__46299 = datafied_throwable;
var map__46299__$1 = (((((!((map__46299 == null))))?(((((map__46299.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46299.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46299):map__46299);
var via = cljs.core.get.call(null,map__46299__$1,new cljs.core.Keyword(null,"via","via",-1904457336));
var trace = cljs.core.get.call(null,map__46299__$1,new cljs.core.Keyword(null,"trace","trace",-1082747415));
var phase = cljs.core.get.call(null,map__46299__$1,new cljs.core.Keyword(null,"phase","phase",575722892),new cljs.core.Keyword(null,"execution","execution",253283524));
var map__46300 = cljs.core.last.call(null,via);
var map__46300__$1 = (((((!((map__46300 == null))))?(((((map__46300.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46300.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46300):map__46300);
var type = cljs.core.get.call(null,map__46300__$1,new cljs.core.Keyword(null,"type","type",1174270348));
var message = cljs.core.get.call(null,map__46300__$1,new cljs.core.Keyword(null,"message","message",-406056002));
var data = cljs.core.get.call(null,map__46300__$1,new cljs.core.Keyword(null,"data","data",-232669377));
var map__46301 = data;
var map__46301__$1 = (((((!((map__46301 == null))))?(((((map__46301.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46301.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46301):map__46301);
var problems = cljs.core.get.call(null,map__46301__$1,new cljs.core.Keyword("cljs.spec.alpha","problems","cljs.spec.alpha/problems",447400814));
var fn = cljs.core.get.call(null,map__46301__$1,new cljs.core.Keyword("cljs.spec.alpha","fn","cljs.spec.alpha/fn",408600443));
var caller = cljs.core.get.call(null,map__46301__$1,new cljs.core.Keyword("cljs.spec.test.alpha","caller","cljs.spec.test.alpha/caller",-398302390));
var map__46302 = new cljs.core.Keyword(null,"data","data",-232669377).cljs$core$IFn$_invoke$arity$1(cljs.core.first.call(null,via));
var map__46302__$1 = (((((!((map__46302 == null))))?(((((map__46302.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46302.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46302):map__46302);
var top_data = map__46302__$1;
var source = cljs.core.get.call(null,map__46302__$1,new cljs.core.Keyword("clojure.error","source","clojure.error/source",-2011936397));
return cljs.core.assoc.call(null,(function (){var G__46307 = phase;
var G__46307__$1 = (((G__46307 instanceof cljs.core.Keyword))?G__46307.fqn:null);
switch (G__46307__$1) {
case "read-source":
var map__46308 = data;
var map__46308__$1 = (((((!((map__46308 == null))))?(((((map__46308.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46308.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46308):map__46308);
var line = cljs.core.get.call(null,map__46308__$1,new cljs.core.Keyword("clojure.error","line","clojure.error/line",-1816287471));
var column = cljs.core.get.call(null,map__46308__$1,new cljs.core.Keyword("clojure.error","column","clojure.error/column",304721553));
var G__46310 = cljs.core.merge.call(null,new cljs.core.Keyword(null,"data","data",-232669377).cljs$core$IFn$_invoke$arity$1(cljs.core.second.call(null,via)),top_data);
var G__46310__$1 = (cljs.core.truth_(source)?cljs.core.assoc.call(null,G__46310,new cljs.core.Keyword("clojure.error","source","clojure.error/source",-2011936397),source):G__46310);
var G__46310__$2 = (cljs.core.truth_(new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 2, ["NO_SOURCE_PATH",null,"NO_SOURCE_FILE",null], null), null).call(null,source))?cljs.core.dissoc.call(null,G__46310__$1,new cljs.core.Keyword("clojure.error","source","clojure.error/source",-2011936397)):G__46310__$1);
if(cljs.core.truth_(message)){
return cljs.core.assoc.call(null,G__46310__$2,new cljs.core.Keyword("clojure.error","cause","clojure.error/cause",-1879175742),message);
} else {
return G__46310__$2;
}

break;
case "compile-syntax-check":
case "compilation":
case "macro-syntax-check":
case "macroexpansion":
var G__46311 = top_data;
var G__46311__$1 = (cljs.core.truth_(source)?cljs.core.assoc.call(null,G__46311,new cljs.core.Keyword("clojure.error","source","clojure.error/source",-2011936397),source):G__46311);
var G__46311__$2 = (cljs.core.truth_(new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 2, ["NO_SOURCE_PATH",null,"NO_SOURCE_FILE",null], null), null).call(null,source))?cljs.core.dissoc.call(null,G__46311__$1,new cljs.core.Keyword("clojure.error","source","clojure.error/source",-2011936397)):G__46311__$1);
var G__46311__$3 = (cljs.core.truth_(type)?cljs.core.assoc.call(null,G__46311__$2,new cljs.core.Keyword("clojure.error","class","clojure.error/class",278435890),type):G__46311__$2);
var G__46311__$4 = (cljs.core.truth_(message)?cljs.core.assoc.call(null,G__46311__$3,new cljs.core.Keyword("clojure.error","cause","clojure.error/cause",-1879175742),message):G__46311__$3);
if(cljs.core.truth_(problems)){
return cljs.core.assoc.call(null,G__46311__$4,new cljs.core.Keyword("clojure.error","spec","clojure.error/spec",2055032595),data);
} else {
return G__46311__$4;
}

break;
case "read-eval-result":
case "print-eval-result":
var vec__46312 = cljs.core.first.call(null,trace);
var source__$1 = cljs.core.nth.call(null,vec__46312,(0),null);
var method = cljs.core.nth.call(null,vec__46312,(1),null);
var file = cljs.core.nth.call(null,vec__46312,(2),null);
var line = cljs.core.nth.call(null,vec__46312,(3),null);
var G__46315 = top_data;
var G__46315__$1 = (cljs.core.truth_(line)?cljs.core.assoc.call(null,G__46315,new cljs.core.Keyword("clojure.error","line","clojure.error/line",-1816287471),line):G__46315);
var G__46315__$2 = (cljs.core.truth_(file)?cljs.core.assoc.call(null,G__46315__$1,new cljs.core.Keyword("clojure.error","source","clojure.error/source",-2011936397),file):G__46315__$1);
var G__46315__$3 = (cljs.core.truth_((function (){var and__4115__auto__ = source__$1;
if(cljs.core.truth_(and__4115__auto__)){
return method;
} else {
return and__4115__auto__;
}
})())?cljs.core.assoc.call(null,G__46315__$2,new cljs.core.Keyword("clojure.error","symbol","clojure.error/symbol",1544821994),(new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[source__$1,method],null))):G__46315__$2);
var G__46315__$4 = (cljs.core.truth_(type)?cljs.core.assoc.call(null,G__46315__$3,new cljs.core.Keyword("clojure.error","class","clojure.error/class",278435890),type):G__46315__$3);
if(cljs.core.truth_(message)){
return cljs.core.assoc.call(null,G__46315__$4,new cljs.core.Keyword("clojure.error","cause","clojure.error/cause",-1879175742),message);
} else {
return G__46315__$4;
}

break;
case "execution":
var vec__46316 = cljs.core.first.call(null,trace);
var source__$1 = cljs.core.nth.call(null,vec__46316,(0),null);
var method = cljs.core.nth.call(null,vec__46316,(1),null);
var file = cljs.core.nth.call(null,vec__46316,(2),null);
var line = cljs.core.nth.call(null,vec__46316,(3),null);
var file__$1 = cljs.core.first.call(null,cljs.core.remove.call(null,(function (p1__46298_SHARP_){
var or__4126__auto__ = (p1__46298_SHARP_ == null);
if(or__4126__auto__){
return or__4126__auto__;
} else {
return new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 2, ["NO_SOURCE_PATH",null,"NO_SOURCE_FILE",null], null), null).call(null,p1__46298_SHARP_);
}
}),new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"file","file",-1269645878).cljs$core$IFn$_invoke$arity$1(caller),file], null)));
var err_line = (function (){var or__4126__auto__ = new cljs.core.Keyword(null,"line","line",212345235).cljs$core$IFn$_invoke$arity$1(caller);
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return line;
}
})();
var G__46319 = new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword("clojure.error","class","clojure.error/class",278435890),type], null);
var G__46319__$1 = (cljs.core.truth_(err_line)?cljs.core.assoc.call(null,G__46319,new cljs.core.Keyword("clojure.error","line","clojure.error/line",-1816287471),err_line):G__46319);
var G__46319__$2 = (cljs.core.truth_(message)?cljs.core.assoc.call(null,G__46319__$1,new cljs.core.Keyword("clojure.error","cause","clojure.error/cause",-1879175742),message):G__46319__$1);
var G__46319__$3 = (cljs.core.truth_((function (){var or__4126__auto__ = fn;
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
var and__4115__auto__ = source__$1;
if(cljs.core.truth_(and__4115__auto__)){
return method;
} else {
return and__4115__auto__;
}
}
})())?cljs.core.assoc.call(null,G__46319__$2,new cljs.core.Keyword("clojure.error","symbol","clojure.error/symbol",1544821994),(function (){var or__4126__auto__ = fn;
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return (new cljs.core.PersistentVector(null,2,(5),cljs.core.PersistentVector.EMPTY_NODE,[source__$1,method],null));
}
})()):G__46319__$2);
var G__46319__$4 = (cljs.core.truth_(file__$1)?cljs.core.assoc.call(null,G__46319__$3,new cljs.core.Keyword("clojure.error","source","clojure.error/source",-2011936397),file__$1):G__46319__$3);
if(cljs.core.truth_(problems)){
return cljs.core.assoc.call(null,G__46319__$4,new cljs.core.Keyword("clojure.error","spec","clojure.error/spec",2055032595),data);
} else {
return G__46319__$4;
}

break;
default:
throw (new Error(["No matching clause: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(G__46307__$1)].join('')));

}
})(),new cljs.core.Keyword("clojure.error","phase","clojure.error/phase",275140358),phase);
});
/**
 * Returns a string from exception data, as produced by ex-triage.
 *   The first line summarizes the exception phase and location.
 *   The subsequent lines describe the cause.
 */
cljs.repl.ex_str = (function cljs$repl$ex_str(p__46323){
var map__46324 = p__46323;
var map__46324__$1 = (((((!((map__46324 == null))))?(((((map__46324.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46324.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46324):map__46324);
var triage_data = map__46324__$1;
var phase = cljs.core.get.call(null,map__46324__$1,new cljs.core.Keyword("clojure.error","phase","clojure.error/phase",275140358));
var source = cljs.core.get.call(null,map__46324__$1,new cljs.core.Keyword("clojure.error","source","clojure.error/source",-2011936397));
var line = cljs.core.get.call(null,map__46324__$1,new cljs.core.Keyword("clojure.error","line","clojure.error/line",-1816287471));
var column = cljs.core.get.call(null,map__46324__$1,new cljs.core.Keyword("clojure.error","column","clojure.error/column",304721553));
var symbol = cljs.core.get.call(null,map__46324__$1,new cljs.core.Keyword("clojure.error","symbol","clojure.error/symbol",1544821994));
var class$ = cljs.core.get.call(null,map__46324__$1,new cljs.core.Keyword("clojure.error","class","clojure.error/class",278435890));
var cause = cljs.core.get.call(null,map__46324__$1,new cljs.core.Keyword("clojure.error","cause","clojure.error/cause",-1879175742));
var spec = cljs.core.get.call(null,map__46324__$1,new cljs.core.Keyword("clojure.error","spec","clojure.error/spec",2055032595));
var loc = [cljs.core.str.cljs$core$IFn$_invoke$arity$1((function (){var or__4126__auto__ = source;
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return "<cljs repl>";
}
})()),":",cljs.core.str.cljs$core$IFn$_invoke$arity$1((function (){var or__4126__auto__ = line;
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return (1);
}
})()),(cljs.core.truth_(column)?[":",cljs.core.str.cljs$core$IFn$_invoke$arity$1(column)].join(''):"")].join('');
var class_name = cljs.core.name.call(null,(function (){var or__4126__auto__ = class$;
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return "";
}
})());
var simple_class = class_name;
var cause_type = ((cljs.core.contains_QMARK_.call(null,new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 2, ["RuntimeException",null,"Exception",null], null), null),simple_class))?"":[" (",simple_class,")"].join(''));
var format = goog.string.format;
var G__46326 = phase;
var G__46326__$1 = (((G__46326 instanceof cljs.core.Keyword))?G__46326.fqn:null);
switch (G__46326__$1) {
case "read-source":
return format.call(null,"Syntax error reading source at (%s).\n%s\n",loc,cause);

break;
case "macro-syntax-check":
return format.call(null,"Syntax error macroexpanding %sat (%s).\n%s",(cljs.core.truth_(symbol)?[cljs.core.str.cljs$core$IFn$_invoke$arity$1(symbol)," "].join(''):""),loc,(cljs.core.truth_(spec)?(function (){var sb__4667__auto__ = (new goog.string.StringBuffer());
var _STAR_print_newline_STAR__orig_val__46327_46336 = cljs.core._STAR_print_newline_STAR_;
var _STAR_print_fn_STAR__orig_val__46328_46337 = cljs.core._STAR_print_fn_STAR_;
var _STAR_print_newline_STAR__temp_val__46329_46338 = true;
var _STAR_print_fn_STAR__temp_val__46330_46339 = (function (x__4668__auto__){
return sb__4667__auto__.append(x__4668__auto__);
});
(cljs.core._STAR_print_newline_STAR_ = _STAR_print_newline_STAR__temp_val__46329_46338);

(cljs.core._STAR_print_fn_STAR_ = _STAR_print_fn_STAR__temp_val__46330_46339);

try{cljs.spec.alpha.explain_out.call(null,cljs.core.update.call(null,spec,new cljs.core.Keyword("cljs.spec.alpha","problems","cljs.spec.alpha/problems",447400814),(function (probs){
return cljs.core.map.call(null,(function (p1__46321_SHARP_){
return cljs.core.dissoc.call(null,p1__46321_SHARP_,new cljs.core.Keyword(null,"in","in",-1531184865));
}),probs);
}))
);
}finally {(cljs.core._STAR_print_fn_STAR_ = _STAR_print_fn_STAR__orig_val__46328_46337);

(cljs.core._STAR_print_newline_STAR_ = _STAR_print_newline_STAR__orig_val__46327_46336);
}
return cljs.core.str.cljs$core$IFn$_invoke$arity$1(sb__4667__auto__);
})():format.call(null,"%s\n",cause)));

break;
case "macroexpansion":
return format.call(null,"Unexpected error%s macroexpanding %sat (%s).\n%s\n",cause_type,(cljs.core.truth_(symbol)?[cljs.core.str.cljs$core$IFn$_invoke$arity$1(symbol)," "].join(''):""),loc,cause);

break;
case "compile-syntax-check":
return format.call(null,"Syntax error%s compiling %sat (%s).\n%s\n",cause_type,(cljs.core.truth_(symbol)?[cljs.core.str.cljs$core$IFn$_invoke$arity$1(symbol)," "].join(''):""),loc,cause);

break;
case "compilation":
return format.call(null,"Unexpected error%s compiling %sat (%s).\n%s\n",cause_type,(cljs.core.truth_(symbol)?[cljs.core.str.cljs$core$IFn$_invoke$arity$1(symbol)," "].join(''):""),loc,cause);

break;
case "read-eval-result":
return format.call(null,"Error reading eval result%s at %s (%s).\n%s\n",cause_type,symbol,loc,cause);

break;
case "print-eval-result":
return format.call(null,"Error printing return value%s at %s (%s).\n%s\n",cause_type,symbol,loc,cause);

break;
case "execution":
if(cljs.core.truth_(spec)){
return format.call(null,"Execution error - invalid arguments to %s at (%s).\n%s",symbol,loc,(function (){var sb__4667__auto__ = (new goog.string.StringBuffer());
var _STAR_print_newline_STAR__orig_val__46331_46340 = cljs.core._STAR_print_newline_STAR_;
var _STAR_print_fn_STAR__orig_val__46332_46341 = cljs.core._STAR_print_fn_STAR_;
var _STAR_print_newline_STAR__temp_val__46333_46342 = true;
var _STAR_print_fn_STAR__temp_val__46334_46343 = (function (x__4668__auto__){
return sb__4667__auto__.append(x__4668__auto__);
});
(cljs.core._STAR_print_newline_STAR_ = _STAR_print_newline_STAR__temp_val__46333_46342);

(cljs.core._STAR_print_fn_STAR_ = _STAR_print_fn_STAR__temp_val__46334_46343);

try{cljs.spec.alpha.explain_out.call(null,cljs.core.update.call(null,spec,new cljs.core.Keyword("cljs.spec.alpha","problems","cljs.spec.alpha/problems",447400814),(function (probs){
return cljs.core.map.call(null,(function (p1__46322_SHARP_){
return cljs.core.dissoc.call(null,p1__46322_SHARP_,new cljs.core.Keyword(null,"in","in",-1531184865));
}),probs);
}))
);
}finally {(cljs.core._STAR_print_fn_STAR_ = _STAR_print_fn_STAR__orig_val__46332_46341);

(cljs.core._STAR_print_newline_STAR_ = _STAR_print_newline_STAR__orig_val__46331_46340);
}
return cljs.core.str.cljs$core$IFn$_invoke$arity$1(sb__4667__auto__);
})());
} else {
return format.call(null,"Execution error%s at %s(%s).\n%s\n",cause_type,(cljs.core.truth_(symbol)?[cljs.core.str.cljs$core$IFn$_invoke$arity$1(symbol)," "].join(''):""),loc,cause);
}

break;
default:
throw (new Error(["No matching clause: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(G__46326__$1)].join('')));

}
});
cljs.repl.error__GT_str = (function cljs$repl$error__GT_str(error){
return cljs.repl.ex_str.call(null,cljs.repl.ex_triage.call(null,cljs.repl.Error__GT_map.call(null,error)));
});

//# sourceMappingURL=repl.js.map?rel=1654487553413
