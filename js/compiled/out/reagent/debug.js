// Compiled by ClojureScript 1.10.773 {}
goog.provide('reagent.debug');
goog.require('cljs.core');
reagent.debug.has_console = (typeof console !== 'undefined');
reagent.debug.tracking = false;
if((typeof reagent !== 'undefined') && (typeof reagent.debug !== 'undefined') && (typeof reagent.debug.warnings !== 'undefined')){
} else {
reagent.debug.warnings = cljs.core.atom.call(null,null);
}
if((typeof reagent !== 'undefined') && (typeof reagent.debug !== 'undefined') && (typeof reagent.debug.track_console !== 'undefined')){
} else {
reagent.debug.track_console = (function (){var o = ({});
(o.warn = (function() { 
var G__39747__delegate = function (args){
return cljs.core.swap_BANG_.call(null,reagent.debug.warnings,cljs.core.update_in,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"warn","warn",-436710552)], null),cljs.core.conj,cljs.core.apply.call(null,cljs.core.str,args));
};
var G__39747 = function (var_args){
var args = null;
if (arguments.length > 0) {
var G__39748__i = 0, G__39748__a = new Array(arguments.length -  0);
while (G__39748__i < G__39748__a.length) {G__39748__a[G__39748__i] = arguments[G__39748__i + 0]; ++G__39748__i;}
  args = new cljs.core.IndexedSeq(G__39748__a,0,null);
} 
return G__39747__delegate.call(this,args);};
G__39747.cljs$lang$maxFixedArity = 0;
G__39747.cljs$lang$applyTo = (function (arglist__39749){
var args = cljs.core.seq(arglist__39749);
return G__39747__delegate(args);
});
G__39747.cljs$core$IFn$_invoke$arity$variadic = G__39747__delegate;
return G__39747;
})()
);

(o.error = (function() { 
var G__39750__delegate = function (args){
return cljs.core.swap_BANG_.call(null,reagent.debug.warnings,cljs.core.update_in,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"error","error",-978969032)], null),cljs.core.conj,cljs.core.apply.call(null,cljs.core.str,args));
};
var G__39750 = function (var_args){
var args = null;
if (arguments.length > 0) {
var G__39751__i = 0, G__39751__a = new Array(arguments.length -  0);
while (G__39751__i < G__39751__a.length) {G__39751__a[G__39751__i] = arguments[G__39751__i + 0]; ++G__39751__i;}
  args = new cljs.core.IndexedSeq(G__39751__a,0,null);
} 
return G__39750__delegate.call(this,args);};
G__39750.cljs$lang$maxFixedArity = 0;
G__39750.cljs$lang$applyTo = (function (arglist__39752){
var args = cljs.core.seq(arglist__39752);
return G__39750__delegate(args);
});
G__39750.cljs$core$IFn$_invoke$arity$variadic = G__39750__delegate;
return G__39750;
})()
);

return o;
})();
}
reagent.debug.track_warnings = (function reagent$debug$track_warnings(f){
(reagent.debug.tracking = true);

cljs.core.reset_BANG_.call(null,reagent.debug.warnings,null);

f.call(null);

var warns = cljs.core.deref.call(null,reagent.debug.warnings);
cljs.core.reset_BANG_.call(null,reagent.debug.warnings,null);

(reagent.debug.tracking = false);

return warns;
});

//# sourceMappingURL=debug.js.map?rel=1654487547784
