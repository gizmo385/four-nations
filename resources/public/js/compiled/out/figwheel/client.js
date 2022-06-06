// Compiled by ClojureScript 1.10.773 {}
goog.provide('figwheel.client');
goog.require('cljs.core');
goog.require('goog.Uri');
goog.require('goog.userAgent.product');
goog.require('goog.object');
goog.require('cljs.reader');
goog.require('cljs.core.async');
goog.require('figwheel.client.socket');
goog.require('figwheel.client.utils');
goog.require('figwheel.client.heads_up');
goog.require('figwheel.client.file_reloading');
goog.require('clojure.string');
goog.require('cljs.repl');
figwheel.client._figwheel_version_ = "0.5.20";
figwheel.client.js_stringify = (((((typeof JSON !== 'undefined')) && ((!((JSON.stringify == null))))))?(function (x){
return ["#js ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(JSON.stringify(x,null," "))].join('');
}):(function (x){
try{return cljs.core.str.cljs$core$IFn$_invoke$arity$1(x);
}catch (e46538){if((e46538 instanceof Error)){
var e = e46538;
return "Error: Unable to stringify";
} else {
throw e46538;

}
}}));
figwheel.client.figwheel_repl_print = (function figwheel$client$figwheel_repl_print(var_args){
var G__46541 = arguments.length;
switch (G__46541) {
case 2:
return figwheel.client.figwheel_repl_print.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 1:
return figwheel.client.figwheel_repl_print.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(figwheel.client.figwheel_repl_print.cljs$core$IFn$_invoke$arity$2 = (function (stream,args){
figwheel.client.socket.send_BANG_.call(null,new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"figwheel-event","figwheel-event",519570592),"callback",new cljs.core.Keyword(null,"callback-name","callback-name",336964714),"figwheel-repl-print",new cljs.core.Keyword(null,"content","content",15833224),new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"stream","stream",1534941648),stream,new cljs.core.Keyword(null,"args","args",1315556576),cljs.core.mapv.call(null,(function (p1__46539_SHARP_){
if(typeof p1__46539_SHARP_ === 'string'){
return p1__46539_SHARP_;
} else {
return figwheel.client.js_stringify.call(null,p1__46539_SHARP_);
}
}),args)], null)], null));

return null;
}));

(figwheel.client.figwheel_repl_print.cljs$core$IFn$_invoke$arity$1 = (function (args){
return figwheel.client.figwheel_repl_print.call(null,new cljs.core.Keyword(null,"out","out",-910545517),args);
}));

(figwheel.client.figwheel_repl_print.cljs$lang$maxFixedArity = 2);

figwheel.client.console_out_print = (function figwheel$client$console_out_print(args){
return console.log.apply(console,cljs.core.into_array.call(null,args));
});
figwheel.client.console_err_print = (function figwheel$client$console_err_print(args){
return console.error.apply(console,cljs.core.into_array.call(null,args));
});
figwheel.client.repl_out_print_fn = (function figwheel$client$repl_out_print_fn(var_args){
var args__4742__auto__ = [];
var len__4736__auto___46544 = arguments.length;
var i__4737__auto___46545 = (0);
while(true){
if((i__4737__auto___46545 < len__4736__auto___46544)){
args__4742__auto__.push((arguments[i__4737__auto___46545]));

var G__46546 = (i__4737__auto___46545 + (1));
i__4737__auto___46545 = G__46546;
continue;
} else {
}
break;
}

var argseq__4743__auto__ = ((((0) < args__4742__auto__.length))?(new cljs.core.IndexedSeq(args__4742__auto__.slice((0)),(0),null)):null);
return figwheel.client.repl_out_print_fn.cljs$core$IFn$_invoke$arity$variadic(argseq__4743__auto__);
});

(figwheel.client.repl_out_print_fn.cljs$core$IFn$_invoke$arity$variadic = (function (args){
figwheel.client.console_out_print.call(null,args);

figwheel.client.figwheel_repl_print.call(null,new cljs.core.Keyword(null,"out","out",-910545517),args);

return null;
}));

(figwheel.client.repl_out_print_fn.cljs$lang$maxFixedArity = (0));

/** @this {Function} */
(figwheel.client.repl_out_print_fn.cljs$lang$applyTo = (function (seq46543){
var self__4724__auto__ = this;
return self__4724__auto__.cljs$core$IFn$_invoke$arity$variadic(cljs.core.seq.call(null,seq46543));
}));

figwheel.client.repl_err_print_fn = (function figwheel$client$repl_err_print_fn(var_args){
var args__4742__auto__ = [];
var len__4736__auto___46548 = arguments.length;
var i__4737__auto___46549 = (0);
while(true){
if((i__4737__auto___46549 < len__4736__auto___46548)){
args__4742__auto__.push((arguments[i__4737__auto___46549]));

var G__46550 = (i__4737__auto___46549 + (1));
i__4737__auto___46549 = G__46550;
continue;
} else {
}
break;
}

var argseq__4743__auto__ = ((((0) < args__4742__auto__.length))?(new cljs.core.IndexedSeq(args__4742__auto__.slice((0)),(0),null)):null);
return figwheel.client.repl_err_print_fn.cljs$core$IFn$_invoke$arity$variadic(argseq__4743__auto__);
});

(figwheel.client.repl_err_print_fn.cljs$core$IFn$_invoke$arity$variadic = (function (args){
figwheel.client.console_err_print.call(null,args);

figwheel.client.figwheel_repl_print.call(null,new cljs.core.Keyword(null,"err","err",-2089457205),args);

return null;
}));

(figwheel.client.repl_err_print_fn.cljs$lang$maxFixedArity = (0));

/** @this {Function} */
(figwheel.client.repl_err_print_fn.cljs$lang$applyTo = (function (seq46547){
var self__4724__auto__ = this;
return self__4724__auto__.cljs$core$IFn$_invoke$arity$variadic(cljs.core.seq.call(null,seq46547));
}));

figwheel.client.enable_repl_print_BANG_ = (function figwheel$client$enable_repl_print_BANG_(){
(cljs.core._STAR_print_newline_STAR_ = false);

cljs.core.set_print_fn_BANG_.call(null,figwheel.client.repl_out_print_fn);

cljs.core.set_print_err_fn_BANG_.call(null,figwheel.client.repl_err_print_fn);

return null;
});
figwheel.client.autoload_QMARK_ = (function figwheel$client$autoload_QMARK_(){
return figwheel.client.utils.persistent_config_get.call(null,new cljs.core.Keyword(null,"figwheel-autoload","figwheel-autoload",-2044741728),true);
});
figwheel.client.toggle_autoload = (function figwheel$client$toggle_autoload(){
var res = figwheel.client.utils.persistent_config_set_BANG_.call(null,new cljs.core.Keyword(null,"figwheel-autoload","figwheel-autoload",-2044741728),cljs.core.not.call(null,figwheel.client.autoload_QMARK_.call(null)));
figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"info","info",-317069002),"Toggle autoload deprecated! Use (figwheel.client/set-autoload! false)");

figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"info","info",-317069002),["Figwheel autoloading ",(cljs.core.truth_(figwheel.client.autoload_QMARK_.call(null))?"ON":"OFF")].join(''));

return res;
});
goog.exportSymbol('figwheel.client.toggle_autoload', figwheel.client.toggle_autoload);
/**
 * Figwheel by default loads code changes as you work. Sometimes you
 *   just want to work on your code without the ramifications of
 *   autoloading and simply load your code piecemeal in the REPL. You can
 *   turn autoloading on and of with this method.
 * 
 *   (figwheel.client/set-autoload false)
 * 
 *   NOTE: This is a persistent setting, meaning that it will persist
 *   through browser reloads.
 */
figwheel.client.set_autoload = (function figwheel$client$set_autoload(b){
if(((b === true) || (b === false))){
} else {
throw (new Error("Assert failed: (or (true? b) (false? b))"));
}

return figwheel.client.utils.persistent_config_set_BANG_.call(null,new cljs.core.Keyword(null,"figwheel-autoload","figwheel-autoload",-2044741728),b);
});
goog.exportSymbol('figwheel.client.set_autoload', figwheel.client.set_autoload);
figwheel.client.repl_pprint = (function figwheel$client$repl_pprint(){
return figwheel.client.utils.persistent_config_get.call(null,new cljs.core.Keyword(null,"figwheel-repl-pprint","figwheel-repl-pprint",1076150873),true);
});
goog.exportSymbol('figwheel.client.repl_pprint', figwheel.client.repl_pprint);
/**
 * This method gives you the ability to turn the pretty printing of
 *   the REPL's return value on and off.
 * 
 *   (figwheel.client/set-repl-pprint false)
 * 
 *   NOTE: This is a persistent setting, meaning that it will persist
 *   through browser reloads.
 */
figwheel.client.set_repl_pprint = (function figwheel$client$set_repl_pprint(b){
if(((b === true) || (b === false))){
} else {
throw (new Error("Assert failed: (or (true? b) (false? b))"));
}

return figwheel.client.utils.persistent_config_set_BANG_.call(null,new cljs.core.Keyword(null,"figwheel-repl-pprint","figwheel-repl-pprint",1076150873),b);
});
goog.exportSymbol('figwheel.client.set_repl_pprint', figwheel.client.set_repl_pprint);
figwheel.client.repl_result_pr_str = (function figwheel$client$repl_result_pr_str(v){
if(cljs.core.truth_(figwheel.client.repl_pprint.call(null))){
return figwheel.client.utils.pprint_to_string.call(null,v);
} else {
return cljs.core.pr_str.call(null,v);
}
});
goog.exportSymbol('figwheel.client.repl_result_pr_str', figwheel.client.repl_result_pr_str);
figwheel.client.get_essential_messages = (function figwheel$client$get_essential_messages(ed){
if(cljs.core.truth_(ed)){
return cljs.core.cons.call(null,cljs.core.select_keys.call(null,ed,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"message","message",-406056002),new cljs.core.Keyword(null,"class","class",-2030961996)], null)),figwheel.client.get_essential_messages.call(null,new cljs.core.Keyword(null,"cause","cause",231901252).cljs$core$IFn$_invoke$arity$1(ed)));
} else {
return null;
}
});
figwheel.client.error_msg_format = (function figwheel$client$error_msg_format(p__46551){
var map__46552 = p__46551;
var map__46552__$1 = (((((!((map__46552 == null))))?(((((map__46552.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46552.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46552):map__46552);
var message = cljs.core.get.call(null,map__46552__$1,new cljs.core.Keyword(null,"message","message",-406056002));
var class$ = cljs.core.get.call(null,map__46552__$1,new cljs.core.Keyword(null,"class","class",-2030961996));
return [cljs.core.str.cljs$core$IFn$_invoke$arity$1(class$)," : ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(message)].join('');
});
figwheel.client.format_messages = cljs.core.comp.call(null,cljs.core.partial.call(null,cljs.core.map,figwheel.client.error_msg_format),figwheel.client.get_essential_messages);
figwheel.client.focus_msgs = (function figwheel$client$focus_msgs(name_set,msg_hist){
return cljs.core.cons.call(null,cljs.core.first.call(null,msg_hist),cljs.core.filter.call(null,cljs.core.comp.call(null,name_set,new cljs.core.Keyword(null,"msg-name","msg-name",-353709863)),cljs.core.rest.call(null,msg_hist)));
});
figwheel.client.reload_file_QMARK__STAR_ = (function figwheel$client$reload_file_QMARK__STAR_(msg_name,opts){
var or__4126__auto__ = new cljs.core.Keyword(null,"load-warninged-code","load-warninged-code",-2030345223).cljs$core$IFn$_invoke$arity$1(opts);
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return cljs.core.not_EQ_.call(null,msg_name,new cljs.core.Keyword(null,"compile-warning","compile-warning",43425356));
}
});
figwheel.client.reload_file_state_QMARK_ = (function figwheel$client$reload_file_state_QMARK_(msg_names,opts){
if(cljs.core._EQ_.call(null,cljs.core.first.call(null,msg_names),new cljs.core.Keyword(null,"files-changed","files-changed",-1418200563))){
return figwheel.client.reload_file_QMARK__STAR_.call(null,cljs.core.second.call(null,msg_names),opts);
} else {
return false;
}
});
figwheel.client.block_reload_file_state_QMARK_ = (function figwheel$client$block_reload_file_state_QMARK_(msg_names,opts){
return ((cljs.core._EQ_.call(null,cljs.core.first.call(null,msg_names),new cljs.core.Keyword(null,"files-changed","files-changed",-1418200563))) && (cljs.core.not.call(null,figwheel.client.reload_file_QMARK__STAR_.call(null,cljs.core.second.call(null,msg_names),opts))));
});
figwheel.client.warning_append_state_QMARK_ = (function figwheel$client$warning_append_state_QMARK_(msg_names){
return cljs.core._EQ_.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"compile-warning","compile-warning",43425356),new cljs.core.Keyword(null,"compile-warning","compile-warning",43425356)], null),cljs.core.take.call(null,(2),msg_names));
});
figwheel.client.warning_state_QMARK_ = (function figwheel$client$warning_state_QMARK_(msg_names){
return cljs.core._EQ_.call(null,new cljs.core.Keyword(null,"compile-warning","compile-warning",43425356),cljs.core.first.call(null,msg_names));
});
figwheel.client.rewarning_state_QMARK_ = (function figwheel$client$rewarning_state_QMARK_(msg_names){
return cljs.core._EQ_.call(null,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"compile-warning","compile-warning",43425356),new cljs.core.Keyword(null,"files-changed","files-changed",-1418200563),new cljs.core.Keyword(null,"compile-warning","compile-warning",43425356)], null),cljs.core.take.call(null,(3),msg_names));
});
figwheel.client.compile_fail_state_QMARK_ = (function figwheel$client$compile_fail_state_QMARK_(msg_names){
return cljs.core._EQ_.call(null,new cljs.core.Keyword(null,"compile-failed","compile-failed",-477639289),cljs.core.first.call(null,msg_names));
});
figwheel.client.compile_refail_state_QMARK_ = (function figwheel$client$compile_refail_state_QMARK_(msg_names){
return cljs.core._EQ_.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"compile-failed","compile-failed",-477639289),new cljs.core.Keyword(null,"compile-failed","compile-failed",-477639289)], null),cljs.core.take.call(null,(2),msg_names));
});
figwheel.client.css_loaded_state_QMARK_ = (function figwheel$client$css_loaded_state_QMARK_(msg_names){
return cljs.core._EQ_.call(null,new cljs.core.Keyword(null,"css-files-changed","css-files-changed",720773874),cljs.core.first.call(null,msg_names));
});
figwheel.client.file_reloader_plugin = (function figwheel$client$file_reloader_plugin(opts){
var ch = cljs.core.async.chan.call(null);
var c__26800__auto___46631 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_46603){
var state_val_46604 = (state_46603[(1)]);
if((state_val_46604 === (7))){
var inst_46599 = (state_46603[(2)]);
var state_46603__$1 = state_46603;
var statearr_46605_46632 = state_46603__$1;
(statearr_46605_46632[(2)] = inst_46599);

(statearr_46605_46632[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (1))){
var state_46603__$1 = state_46603;
var statearr_46606_46633 = state_46603__$1;
(statearr_46606_46633[(2)] = null);

(statearr_46606_46633[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (4))){
var inst_46556 = (state_46603[(7)]);
var inst_46556__$1 = (state_46603[(2)]);
var state_46603__$1 = (function (){var statearr_46607 = state_46603;
(statearr_46607[(7)] = inst_46556__$1);

return statearr_46607;
})();
if(cljs.core.truth_(inst_46556__$1)){
var statearr_46608_46634 = state_46603__$1;
(statearr_46608_46634[(1)] = (5));

} else {
var statearr_46609_46635 = state_46603__$1;
(statearr_46609_46635[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (15))){
var inst_46563 = (state_46603[(8)]);
var inst_46578 = new cljs.core.Keyword(null,"files","files",-472457450).cljs$core$IFn$_invoke$arity$1(inst_46563);
var inst_46579 = cljs.core.first.call(null,inst_46578);
var inst_46580 = new cljs.core.Keyword(null,"file","file",-1269645878).cljs$core$IFn$_invoke$arity$1(inst_46579);
var inst_46581 = ["Figwheel: Not loading code with warnings - ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(inst_46580)].join('');
var inst_46582 = figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"warn","warn",-436710552),inst_46581);
var state_46603__$1 = state_46603;
var statearr_46610_46636 = state_46603__$1;
(statearr_46610_46636[(2)] = inst_46582);

(statearr_46610_46636[(1)] = (17));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (13))){
var inst_46587 = (state_46603[(2)]);
var state_46603__$1 = state_46603;
var statearr_46611_46637 = state_46603__$1;
(statearr_46611_46637[(2)] = inst_46587);

(statearr_46611_46637[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (6))){
var state_46603__$1 = state_46603;
var statearr_46612_46638 = state_46603__$1;
(statearr_46612_46638[(2)] = null);

(statearr_46612_46638[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (17))){
var inst_46585 = (state_46603[(2)]);
var state_46603__$1 = state_46603;
var statearr_46613_46639 = state_46603__$1;
(statearr_46613_46639[(2)] = inst_46585);

(statearr_46613_46639[(1)] = (13));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (3))){
var inst_46601 = (state_46603[(2)]);
var state_46603__$1 = state_46603;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_46603__$1,inst_46601);
} else {
if((state_val_46604 === (12))){
var inst_46562 = (state_46603[(9)]);
var inst_46576 = figwheel.client.block_reload_file_state_QMARK_.call(null,inst_46562,opts);
var state_46603__$1 = state_46603;
if(inst_46576){
var statearr_46614_46640 = state_46603__$1;
(statearr_46614_46640[(1)] = (15));

} else {
var statearr_46615_46641 = state_46603__$1;
(statearr_46615_46641[(1)] = (16));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (2))){
var state_46603__$1 = state_46603;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46603__$1,(4),ch);
} else {
if((state_val_46604 === (11))){
var inst_46563 = (state_46603[(8)]);
var inst_46568 = cljs.core.PersistentVector.EMPTY_NODE;
var inst_46569 = figwheel.client.file_reloading.reload_js_files.call(null,opts,inst_46563);
var inst_46570 = cljs.core.async.timeout.call(null,(1000));
var inst_46571 = [inst_46569,inst_46570];
var inst_46572 = (new cljs.core.PersistentVector(null,2,(5),inst_46568,inst_46571,null));
var state_46603__$1 = state_46603;
return cljs.core.async.ioc_alts_BANG_.call(null,state_46603__$1,(14),inst_46572);
} else {
if((state_val_46604 === (9))){
var inst_46563 = (state_46603[(8)]);
var inst_46589 = figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"warn","warn",-436710552),"Figwheel: code autoloading is OFF");
var inst_46590 = new cljs.core.Keyword(null,"files","files",-472457450).cljs$core$IFn$_invoke$arity$1(inst_46563);
var inst_46591 = cljs.core.map.call(null,new cljs.core.Keyword(null,"file","file",-1269645878),inst_46590);
var inst_46592 = ["Not loading: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(inst_46591)].join('');
var inst_46593 = figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"info","info",-317069002),inst_46592);
var state_46603__$1 = (function (){var statearr_46616 = state_46603;
(statearr_46616[(10)] = inst_46589);

return statearr_46616;
})();
var statearr_46617_46642 = state_46603__$1;
(statearr_46617_46642[(2)] = inst_46593);

(statearr_46617_46642[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (5))){
var inst_46556 = (state_46603[(7)]);
var inst_46558 = [new cljs.core.Keyword(null,"compile-warning","compile-warning",43425356),null,new cljs.core.Keyword(null,"files-changed","files-changed",-1418200563),null];
var inst_46559 = (new cljs.core.PersistentArrayMap(null,2,inst_46558,null));
var inst_46560 = (new cljs.core.PersistentHashSet(null,inst_46559,null));
var inst_46561 = figwheel.client.focus_msgs.call(null,inst_46560,inst_46556);
var inst_46562 = cljs.core.map.call(null,new cljs.core.Keyword(null,"msg-name","msg-name",-353709863),inst_46561);
var inst_46563 = cljs.core.first.call(null,inst_46561);
var inst_46564 = figwheel.client.autoload_QMARK_.call(null);
var state_46603__$1 = (function (){var statearr_46618 = state_46603;
(statearr_46618[(8)] = inst_46563);

(statearr_46618[(9)] = inst_46562);

return statearr_46618;
})();
if(cljs.core.truth_(inst_46564)){
var statearr_46619_46643 = state_46603__$1;
(statearr_46619_46643[(1)] = (8));

} else {
var statearr_46620_46644 = state_46603__$1;
(statearr_46620_46644[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (14))){
var inst_46574 = (state_46603[(2)]);
var state_46603__$1 = state_46603;
var statearr_46621_46645 = state_46603__$1;
(statearr_46621_46645[(2)] = inst_46574);

(statearr_46621_46645[(1)] = (13));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (16))){
var state_46603__$1 = state_46603;
var statearr_46622_46646 = state_46603__$1;
(statearr_46622_46646[(2)] = null);

(statearr_46622_46646[(1)] = (17));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (10))){
var inst_46595 = (state_46603[(2)]);
var state_46603__$1 = (function (){var statearr_46623 = state_46603;
(statearr_46623[(11)] = inst_46595);

return statearr_46623;
})();
var statearr_46624_46647 = state_46603__$1;
(statearr_46624_46647[(2)] = null);

(statearr_46624_46647[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46604 === (8))){
var inst_46562 = (state_46603[(9)]);
var inst_46566 = figwheel.client.reload_file_state_QMARK_.call(null,inst_46562,opts);
var state_46603__$1 = state_46603;
if(cljs.core.truth_(inst_46566)){
var statearr_46625_46648 = state_46603__$1;
(statearr_46625_46648[(1)] = (11));

} else {
var statearr_46626_46649 = state_46603__$1;
(statearr_46626_46649[(1)] = (12));

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
});
return (function() {
var figwheel$client$file_reloader_plugin_$_state_machine__26706__auto__ = null;
var figwheel$client$file_reloader_plugin_$_state_machine__26706__auto____0 = (function (){
var statearr_46627 = [null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_46627[(0)] = figwheel$client$file_reloader_plugin_$_state_machine__26706__auto__);

(statearr_46627[(1)] = (1));

return statearr_46627;
});
var figwheel$client$file_reloader_plugin_$_state_machine__26706__auto____1 = (function (state_46603){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_46603);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e46628){if((e46628 instanceof Object)){
var ex__26709__auto__ = e46628;
var statearr_46629_46650 = state_46603;
(statearr_46629_46650[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_46603);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e46628;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__46651 = state_46603;
state_46603 = G__46651;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
figwheel$client$file_reloader_plugin_$_state_machine__26706__auto__ = function(state_46603){
switch(arguments.length){
case 0:
return figwheel$client$file_reloader_plugin_$_state_machine__26706__auto____0.call(this);
case 1:
return figwheel$client$file_reloader_plugin_$_state_machine__26706__auto____1.call(this,state_46603);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
figwheel$client$file_reloader_plugin_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = figwheel$client$file_reloader_plugin_$_state_machine__26706__auto____0;
figwheel$client$file_reloader_plugin_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = figwheel$client$file_reloader_plugin_$_state_machine__26706__auto____1;
return figwheel$client$file_reloader_plugin_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_46630 = f__26801__auto__.call(null);
(statearr_46630[(6)] = c__26800__auto___46631);

return statearr_46630;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return (function (msg_hist){
cljs.core.async.put_BANG_.call(null,ch,msg_hist);

return msg_hist;
});
});
figwheel.client.truncate_stack_trace = (function figwheel$client$truncate_stack_trace(stack_str){
return cljs.core.take_while.call(null,(function (p1__46652_SHARP_){
return cljs.core.not.call(null,cljs.core.re_matches.call(null,/.*eval_javascript_STAR__STAR_.*/,p1__46652_SHARP_));
}),clojure.string.split_lines.call(null,stack_str));
});
figwheel.client.get_ua_product = (function figwheel$client$get_ua_product(){
if(figwheel.client.utils.node_env_QMARK_.call(null)){
return new cljs.core.Keyword(null,"chrome","chrome",1718738387);
} else {
if(goog.userAgent.product.SAFARI){
return new cljs.core.Keyword(null,"safari","safari",497115653);
} else {
if(goog.userAgent.product.CHROME){
return new cljs.core.Keyword(null,"chrome","chrome",1718738387);
} else {
if(goog.userAgent.product.FIREFOX){
return new cljs.core.Keyword(null,"firefox","firefox",1283768880);
} else {
if(goog.userAgent.product.IE){
return new cljs.core.Keyword(null,"ie","ie",2038473780);
} else {
return null;
}
}
}
}
}
});
var base_path_46658 = figwheel.client.utils.base_url_path.call(null);
figwheel.client.eval_javascript_STAR__STAR_ = (function figwheel$client$eval_javascript_STAR__STAR_(code,opts,result_handler){
try{var sb = (new goog.string.StringBuffer());
var _STAR_print_newline_STAR__orig_val__46654 = cljs.core._STAR_print_newline_STAR_;
var _STAR_print_fn_STAR__orig_val__46655 = cljs.core._STAR_print_fn_STAR_;
var _STAR_print_newline_STAR__temp_val__46656 = true;
var _STAR_print_fn_STAR__temp_val__46657 = (function (x){
return sb.append(x);
});
(cljs.core._STAR_print_newline_STAR_ = _STAR_print_newline_STAR__temp_val__46656);

(cljs.core._STAR_print_fn_STAR_ = _STAR_print_fn_STAR__temp_val__46657);

try{var result_value = figwheel.client.utils.eval_helper.call(null,code,opts);
var result_value__$1 = (((!(typeof result_value === 'string')))?cljs.core.pr_str.call(null,result_value):result_value);
return result_handler.call(null,new cljs.core.PersistentArrayMap(null, 4, [new cljs.core.Keyword(null,"status","status",-1997798413),new cljs.core.Keyword(null,"success","success",1890645906),new cljs.core.Keyword(null,"out","out",-910545517),cljs.core.str.cljs$core$IFn$_invoke$arity$1(sb),new cljs.core.Keyword(null,"ua-product","ua-product",938384227),figwheel.client.get_ua_product.call(null),new cljs.core.Keyword(null,"value","value",305978217),result_value__$1], null));
}finally {(cljs.core._STAR_print_fn_STAR_ = _STAR_print_fn_STAR__orig_val__46655);

(cljs.core._STAR_print_newline_STAR_ = _STAR_print_newline_STAR__orig_val__46654);
}}catch (e46653){if((e46653 instanceof Error)){
var e = e46653;
return result_handler.call(null,new cljs.core.PersistentArrayMap(null, 5, [new cljs.core.Keyword(null,"status","status",-1997798413),new cljs.core.Keyword(null,"exception","exception",-335277064),new cljs.core.Keyword(null,"value","value",305978217),cljs.core.pr_str.call(null,e),new cljs.core.Keyword(null,"ua-product","ua-product",938384227),figwheel.client.get_ua_product.call(null),new cljs.core.Keyword(null,"stacktrace","stacktrace",-95588394),clojure.string.join.call(null,"\n",figwheel.client.truncate_stack_trace.call(null,e.stack)),new cljs.core.Keyword(null,"base-path","base-path",495760020),base_path_46658], null));
} else {
var e = e46653;
return result_handler.call(null,new cljs.core.PersistentArrayMap(null, 4, [new cljs.core.Keyword(null,"status","status",-1997798413),new cljs.core.Keyword(null,"exception","exception",-335277064),new cljs.core.Keyword(null,"ua-product","ua-product",938384227),figwheel.client.get_ua_product.call(null),new cljs.core.Keyword(null,"value","value",305978217),cljs.core.pr_str.call(null,e),new cljs.core.Keyword(null,"stacktrace","stacktrace",-95588394),"No stacktrace available."], null));

}
}});
/**
 * The REPL can disconnect and reconnect lets ensure cljs.user exists at least.
 */
figwheel.client.ensure_cljs_user = (function figwheel$client$ensure_cljs_user(){
if(cljs.core.truth_(cljs.user)){
return null;
} else {
return (cljs.user = ({}));
}
});
figwheel.client.repl_plugin = (function figwheel$client$repl_plugin(p__46659){
var map__46660 = p__46659;
var map__46660__$1 = (((((!((map__46660 == null))))?(((((map__46660.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46660.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46660):map__46660);
var opts = map__46660__$1;
var build_id = cljs.core.get.call(null,map__46660__$1,new cljs.core.Keyword(null,"build-id","build-id",1642831089));
return (function (p__46662){
var vec__46663 = p__46662;
var seq__46664 = cljs.core.seq.call(null,vec__46663);
var first__46665 = cljs.core.first.call(null,seq__46664);
var seq__46664__$1 = cljs.core.next.call(null,seq__46664);
var map__46666 = first__46665;
var map__46666__$1 = (((((!((map__46666 == null))))?(((((map__46666.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46666.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46666):map__46666);
var msg = map__46666__$1;
var msg_name = cljs.core.get.call(null,map__46666__$1,new cljs.core.Keyword(null,"msg-name","msg-name",-353709863));
var _ = seq__46664__$1;
if(cljs.core._EQ_.call(null,new cljs.core.Keyword(null,"repl-eval","repl-eval",-1784727398),msg_name)){
figwheel.client.ensure_cljs_user.call(null);

return figwheel.client.eval_javascript_STAR__STAR_.call(null,new cljs.core.Keyword(null,"code","code",1586293142).cljs$core$IFn$_invoke$arity$1(msg),opts,(function (res){
return figwheel.client.socket.send_BANG_.call(null,new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"figwheel-event","figwheel-event",519570592),"callback",new cljs.core.Keyword(null,"callback-name","callback-name",336964714),new cljs.core.Keyword(null,"callback-name","callback-name",336964714).cljs$core$IFn$_invoke$arity$1(msg),new cljs.core.Keyword(null,"content","content",15833224),res], null));
}));
} else {
return null;
}
});
});
figwheel.client.css_reloader_plugin = (function figwheel$client$css_reloader_plugin(opts){
return (function (p__46668){
var vec__46669 = p__46668;
var seq__46670 = cljs.core.seq.call(null,vec__46669);
var first__46671 = cljs.core.first.call(null,seq__46670);
var seq__46670__$1 = cljs.core.next.call(null,seq__46670);
var map__46672 = first__46671;
var map__46672__$1 = (((((!((map__46672 == null))))?(((((map__46672.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46672.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46672):map__46672);
var msg = map__46672__$1;
var msg_name = cljs.core.get.call(null,map__46672__$1,new cljs.core.Keyword(null,"msg-name","msg-name",-353709863));
var _ = seq__46670__$1;
if(cljs.core._EQ_.call(null,msg_name,new cljs.core.Keyword(null,"css-files-changed","css-files-changed",720773874))){
return figwheel.client.file_reloading.reload_css_files.call(null,opts,msg);
} else {
return null;
}
});
});
figwheel.client.compile_fail_warning_plugin = (function figwheel$client$compile_fail_warning_plugin(p__46674){
var map__46675 = p__46674;
var map__46675__$1 = (((((!((map__46675 == null))))?(((((map__46675.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46675.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46675):map__46675);
var on_compile_warning = cljs.core.get.call(null,map__46675__$1,new cljs.core.Keyword(null,"on-compile-warning","on-compile-warning",-1195585947));
var on_compile_fail = cljs.core.get.call(null,map__46675__$1,new cljs.core.Keyword(null,"on-compile-fail","on-compile-fail",728013036));
return (function (p__46677){
var vec__46678 = p__46677;
var seq__46679 = cljs.core.seq.call(null,vec__46678);
var first__46680 = cljs.core.first.call(null,seq__46679);
var seq__46679__$1 = cljs.core.next.call(null,seq__46679);
var map__46681 = first__46680;
var map__46681__$1 = (((((!((map__46681 == null))))?(((((map__46681.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46681.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46681):map__46681);
var msg = map__46681__$1;
var msg_name = cljs.core.get.call(null,map__46681__$1,new cljs.core.Keyword(null,"msg-name","msg-name",-353709863));
var _ = seq__46679__$1;
var pred__46683 = cljs.core._EQ_;
var expr__46684 = msg_name;
if(cljs.core.truth_(pred__46683.call(null,new cljs.core.Keyword(null,"compile-warning","compile-warning",43425356),expr__46684))){
return on_compile_warning.call(null,msg);
} else {
if(cljs.core.truth_(pred__46683.call(null,new cljs.core.Keyword(null,"compile-failed","compile-failed",-477639289),expr__46684))){
return on_compile_fail.call(null,msg);
} else {
return null;
}
}
});
});
figwheel.client.auto_jump_to_error = (function figwheel$client$auto_jump_to_error(opts,error){
if(cljs.core.truth_(new cljs.core.Keyword(null,"auto-jump-to-source-on-error","auto-jump-to-source-on-error",-960314920).cljs$core$IFn$_invoke$arity$1(opts))){
return figwheel.client.heads_up.auto_notify_source_file_line.call(null,error);
} else {
return null;
}
});
figwheel.client.heads_up_plugin_msg_handler = (function figwheel$client$heads_up_plugin_msg_handler(opts,msg_hist_SINGLEQUOTE_){
var msg_hist = figwheel.client.focus_msgs.call(null,new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"compile-failed","compile-failed",-477639289),null,new cljs.core.Keyword(null,"compile-warning","compile-warning",43425356),null,new cljs.core.Keyword(null,"files-changed","files-changed",-1418200563),null], null), null),msg_hist_SINGLEQUOTE_);
var msg_names = cljs.core.map.call(null,new cljs.core.Keyword(null,"msg-name","msg-name",-353709863),msg_hist);
var msg = cljs.core.first.call(null,msg_hist);
var c__26800__auto__ = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_46773){
var state_val_46774 = (state_46773[(1)]);
if((state_val_46774 === (7))){
var inst_46693 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
if(cljs.core.truth_(inst_46693)){
var statearr_46775_46822 = state_46773__$1;
(statearr_46775_46822[(1)] = (8));

} else {
var statearr_46776_46823 = state_46773__$1;
(statearr_46776_46823[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (20))){
var inst_46767 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
var statearr_46777_46824 = state_46773__$1;
(statearr_46777_46824[(2)] = inst_46767);

(statearr_46777_46824[(1)] = (15));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (27))){
var inst_46763 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
var statearr_46778_46825 = state_46773__$1;
(statearr_46778_46825[(2)] = inst_46763);

(statearr_46778_46825[(1)] = (24));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (1))){
var inst_46686 = figwheel.client.reload_file_state_QMARK_.call(null,msg_names,opts);
var state_46773__$1 = state_46773;
if(cljs.core.truth_(inst_46686)){
var statearr_46779_46826 = state_46773__$1;
(statearr_46779_46826[(1)] = (2));

} else {
var statearr_46780_46827 = state_46773__$1;
(statearr_46780_46827[(1)] = (3));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (24))){
var inst_46765 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
var statearr_46781_46828 = state_46773__$1;
(statearr_46781_46828[(2)] = inst_46765);

(statearr_46781_46828[(1)] = (20));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (4))){
var inst_46771 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_46773__$1,inst_46771);
} else {
if((state_val_46774 === (15))){
var inst_46769 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
var statearr_46782_46829 = state_46773__$1;
(statearr_46782_46829[(2)] = inst_46769);

(statearr_46782_46829[(1)] = (4));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (21))){
var inst_46722 = (state_46773[(2)]);
var inst_46723 = new cljs.core.Keyword(null,"exception-data","exception-data",-512474886).cljs$core$IFn$_invoke$arity$1(msg);
var inst_46724 = figwheel.client.auto_jump_to_error.call(null,opts,inst_46723);
var state_46773__$1 = (function (){var statearr_46783 = state_46773;
(statearr_46783[(7)] = inst_46722);

return statearr_46783;
})();
var statearr_46784_46830 = state_46773__$1;
(statearr_46784_46830[(2)] = inst_46724);

(statearr_46784_46830[(1)] = (20));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (31))){
var inst_46752 = figwheel.client.css_loaded_state_QMARK_.call(null,msg_names);
var state_46773__$1 = state_46773;
if(inst_46752){
var statearr_46785_46831 = state_46773__$1;
(statearr_46785_46831[(1)] = (34));

} else {
var statearr_46786_46832 = state_46773__$1;
(statearr_46786_46832[(1)] = (35));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (32))){
var inst_46761 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
var statearr_46787_46833 = state_46773__$1;
(statearr_46787_46833[(2)] = inst_46761);

(statearr_46787_46833[(1)] = (27));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (33))){
var inst_46748 = (state_46773[(2)]);
var inst_46749 = new cljs.core.Keyword(null,"message","message",-406056002).cljs$core$IFn$_invoke$arity$1(msg);
var inst_46750 = figwheel.client.auto_jump_to_error.call(null,opts,inst_46749);
var state_46773__$1 = (function (){var statearr_46788 = state_46773;
(statearr_46788[(8)] = inst_46748);

return statearr_46788;
})();
var statearr_46789_46834 = state_46773__$1;
(statearr_46789_46834[(2)] = inst_46750);

(statearr_46789_46834[(1)] = (32));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (13))){
var inst_46707 = figwheel.client.heads_up.clear.call(null);
var state_46773__$1 = state_46773;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46773__$1,(16),inst_46707);
} else {
if((state_val_46774 === (22))){
var inst_46728 = new cljs.core.Keyword(null,"message","message",-406056002).cljs$core$IFn$_invoke$arity$1(msg);
var inst_46729 = figwheel.client.heads_up.append_warning_message.call(null,inst_46728);
var state_46773__$1 = state_46773;
var statearr_46790_46835 = state_46773__$1;
(statearr_46790_46835[(2)] = inst_46729);

(statearr_46790_46835[(1)] = (24));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (36))){
var inst_46759 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
var statearr_46791_46836 = state_46773__$1;
(statearr_46791_46836[(2)] = inst_46759);

(statearr_46791_46836[(1)] = (32));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (29))){
var inst_46739 = (state_46773[(2)]);
var inst_46740 = new cljs.core.Keyword(null,"message","message",-406056002).cljs$core$IFn$_invoke$arity$1(msg);
var inst_46741 = figwheel.client.auto_jump_to_error.call(null,opts,inst_46740);
var state_46773__$1 = (function (){var statearr_46792 = state_46773;
(statearr_46792[(9)] = inst_46739);

return statearr_46792;
})();
var statearr_46793_46837 = state_46773__$1;
(statearr_46793_46837[(2)] = inst_46741);

(statearr_46793_46837[(1)] = (27));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (6))){
var inst_46688 = (state_46773[(10)]);
var state_46773__$1 = state_46773;
var statearr_46794_46838 = state_46773__$1;
(statearr_46794_46838[(2)] = inst_46688);

(statearr_46794_46838[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (28))){
var inst_46735 = (state_46773[(2)]);
var inst_46736 = new cljs.core.Keyword(null,"message","message",-406056002).cljs$core$IFn$_invoke$arity$1(msg);
var inst_46737 = figwheel.client.heads_up.display_warning.call(null,inst_46736);
var state_46773__$1 = (function (){var statearr_46795 = state_46773;
(statearr_46795[(11)] = inst_46735);

return statearr_46795;
})();
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46773__$1,(29),inst_46737);
} else {
if((state_val_46774 === (25))){
var inst_46733 = figwheel.client.heads_up.clear.call(null);
var state_46773__$1 = state_46773;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46773__$1,(28),inst_46733);
} else {
if((state_val_46774 === (34))){
var inst_46754 = figwheel.client.heads_up.flash_loaded.call(null);
var state_46773__$1 = state_46773;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46773__$1,(37),inst_46754);
} else {
if((state_val_46774 === (17))){
var inst_46713 = (state_46773[(2)]);
var inst_46714 = new cljs.core.Keyword(null,"exception-data","exception-data",-512474886).cljs$core$IFn$_invoke$arity$1(msg);
var inst_46715 = figwheel.client.auto_jump_to_error.call(null,opts,inst_46714);
var state_46773__$1 = (function (){var statearr_46796 = state_46773;
(statearr_46796[(12)] = inst_46713);

return statearr_46796;
})();
var statearr_46797_46839 = state_46773__$1;
(statearr_46797_46839[(2)] = inst_46715);

(statearr_46797_46839[(1)] = (15));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (3))){
var inst_46705 = figwheel.client.compile_refail_state_QMARK_.call(null,msg_names);
var state_46773__$1 = state_46773;
if(inst_46705){
var statearr_46798_46840 = state_46773__$1;
(statearr_46798_46840[(1)] = (13));

} else {
var statearr_46799_46841 = state_46773__$1;
(statearr_46799_46841[(1)] = (14));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (12))){
var inst_46701 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
var statearr_46800_46842 = state_46773__$1;
(statearr_46800_46842[(2)] = inst_46701);

(statearr_46800_46842[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (2))){
var inst_46688 = (state_46773[(10)]);
var inst_46688__$1 = figwheel.client.autoload_QMARK_.call(null);
var state_46773__$1 = (function (){var statearr_46801 = state_46773;
(statearr_46801[(10)] = inst_46688__$1);

return statearr_46801;
})();
if(cljs.core.truth_(inst_46688__$1)){
var statearr_46802_46843 = state_46773__$1;
(statearr_46802_46843[(1)] = (5));

} else {
var statearr_46803_46844 = state_46773__$1;
(statearr_46803_46844[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (23))){
var inst_46731 = figwheel.client.rewarning_state_QMARK_.call(null,msg_names);
var state_46773__$1 = state_46773;
if(inst_46731){
var statearr_46804_46845 = state_46773__$1;
(statearr_46804_46845[(1)] = (25));

} else {
var statearr_46805_46846 = state_46773__$1;
(statearr_46805_46846[(1)] = (26));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (35))){
var state_46773__$1 = state_46773;
var statearr_46806_46847 = state_46773__$1;
(statearr_46806_46847[(2)] = null);

(statearr_46806_46847[(1)] = (36));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (19))){
var inst_46726 = figwheel.client.warning_append_state_QMARK_.call(null,msg_names);
var state_46773__$1 = state_46773;
if(inst_46726){
var statearr_46807_46848 = state_46773__$1;
(statearr_46807_46848[(1)] = (22));

} else {
var statearr_46808_46849 = state_46773__$1;
(statearr_46808_46849[(1)] = (23));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (11))){
var inst_46697 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
var statearr_46809_46850 = state_46773__$1;
(statearr_46809_46850[(2)] = inst_46697);

(statearr_46809_46850[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (9))){
var inst_46699 = figwheel.client.heads_up.clear.call(null);
var state_46773__$1 = state_46773;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46773__$1,(12),inst_46699);
} else {
if((state_val_46774 === (5))){
var inst_46690 = new cljs.core.Keyword(null,"autoload","autoload",-354122500).cljs$core$IFn$_invoke$arity$1(opts);
var state_46773__$1 = state_46773;
var statearr_46810_46851 = state_46773__$1;
(statearr_46810_46851[(2)] = inst_46690);

(statearr_46810_46851[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (14))){
var inst_46717 = figwheel.client.compile_fail_state_QMARK_.call(null,msg_names);
var state_46773__$1 = state_46773;
if(inst_46717){
var statearr_46811_46852 = state_46773__$1;
(statearr_46811_46852[(1)] = (18));

} else {
var statearr_46812_46853 = state_46773__$1;
(statearr_46812_46853[(1)] = (19));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (26))){
var inst_46743 = figwheel.client.warning_state_QMARK_.call(null,msg_names);
var state_46773__$1 = state_46773;
if(inst_46743){
var statearr_46813_46854 = state_46773__$1;
(statearr_46813_46854[(1)] = (30));

} else {
var statearr_46814_46855 = state_46773__$1;
(statearr_46814_46855[(1)] = (31));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (16))){
var inst_46709 = (state_46773[(2)]);
var inst_46710 = new cljs.core.Keyword(null,"exception-data","exception-data",-512474886).cljs$core$IFn$_invoke$arity$1(msg);
var inst_46711 = figwheel.client.heads_up.display_exception.call(null,inst_46710);
var state_46773__$1 = (function (){var statearr_46815 = state_46773;
(statearr_46815[(13)] = inst_46709);

return statearr_46815;
})();
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46773__$1,(17),inst_46711);
} else {
if((state_val_46774 === (30))){
var inst_46745 = new cljs.core.Keyword(null,"message","message",-406056002).cljs$core$IFn$_invoke$arity$1(msg);
var inst_46746 = figwheel.client.heads_up.display_warning.call(null,inst_46745);
var state_46773__$1 = state_46773;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46773__$1,(33),inst_46746);
} else {
if((state_val_46774 === (10))){
var inst_46703 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
var statearr_46816_46856 = state_46773__$1;
(statearr_46816_46856[(2)] = inst_46703);

(statearr_46816_46856[(1)] = (4));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (18))){
var inst_46719 = new cljs.core.Keyword(null,"exception-data","exception-data",-512474886).cljs$core$IFn$_invoke$arity$1(msg);
var inst_46720 = figwheel.client.heads_up.display_exception.call(null,inst_46719);
var state_46773__$1 = state_46773;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46773__$1,(21),inst_46720);
} else {
if((state_val_46774 === (37))){
var inst_46756 = (state_46773[(2)]);
var state_46773__$1 = state_46773;
var statearr_46817_46857 = state_46773__$1;
(statearr_46817_46857[(2)] = inst_46756);

(statearr_46817_46857[(1)] = (36));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46774 === (8))){
var inst_46695 = figwheel.client.heads_up.flash_loaded.call(null);
var state_46773__$1 = state_46773;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46773__$1,(11),inst_46695);
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
});
return (function() {
var figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto__ = null;
var figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto____0 = (function (){
var statearr_46818 = [null,null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_46818[(0)] = figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto__);

(statearr_46818[(1)] = (1));

return statearr_46818;
});
var figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto____1 = (function (state_46773){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_46773);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e46819){if((e46819 instanceof Object)){
var ex__26709__auto__ = e46819;
var statearr_46820_46858 = state_46773;
(statearr_46820_46858[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_46773);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e46819;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__46859 = state_46773;
state_46773 = G__46859;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto__ = function(state_46773){
switch(arguments.length){
case 0:
return figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto____0.call(this);
case 1:
return figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto____1.call(this,state_46773);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto____0;
figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto____1;
return figwheel$client$heads_up_plugin_msg_handler_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_46821 = f__26801__auto__.call(null);
(statearr_46821[(6)] = c__26800__auto__);

return statearr_46821;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));

return c__26800__auto__;
});
figwheel.client.heads_up_plugin = (function figwheel$client$heads_up_plugin(opts){
var ch = cljs.core.async.chan.call(null);
figwheel.client.heads_up_config_options_STAR__STAR_ = opts;

var c__26800__auto___46888 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_46874){
var state_val_46875 = (state_46874[(1)]);
if((state_val_46875 === (1))){
var state_46874__$1 = state_46874;
var statearr_46876_46889 = state_46874__$1;
(statearr_46876_46889[(2)] = null);

(statearr_46876_46889[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46875 === (2))){
var state_46874__$1 = state_46874;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46874__$1,(4),ch);
} else {
if((state_val_46875 === (3))){
var inst_46872 = (state_46874[(2)]);
var state_46874__$1 = state_46874;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_46874__$1,inst_46872);
} else {
if((state_val_46875 === (4))){
var inst_46862 = (state_46874[(7)]);
var inst_46862__$1 = (state_46874[(2)]);
var state_46874__$1 = (function (){var statearr_46877 = state_46874;
(statearr_46877[(7)] = inst_46862__$1);

return statearr_46877;
})();
if(cljs.core.truth_(inst_46862__$1)){
var statearr_46878_46890 = state_46874__$1;
(statearr_46878_46890[(1)] = (5));

} else {
var statearr_46879_46891 = state_46874__$1;
(statearr_46879_46891[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46875 === (5))){
var inst_46862 = (state_46874[(7)]);
var inst_46864 = figwheel.client.heads_up_plugin_msg_handler.call(null,opts,inst_46862);
var state_46874__$1 = state_46874;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46874__$1,(8),inst_46864);
} else {
if((state_val_46875 === (6))){
var state_46874__$1 = state_46874;
var statearr_46880_46892 = state_46874__$1;
(statearr_46880_46892[(2)] = null);

(statearr_46880_46892[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46875 === (7))){
var inst_46870 = (state_46874[(2)]);
var state_46874__$1 = state_46874;
var statearr_46881_46893 = state_46874__$1;
(statearr_46881_46893[(2)] = inst_46870);

(statearr_46881_46893[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_46875 === (8))){
var inst_46866 = (state_46874[(2)]);
var state_46874__$1 = (function (){var statearr_46882 = state_46874;
(statearr_46882[(8)] = inst_46866);

return statearr_46882;
})();
var statearr_46883_46894 = state_46874__$1;
(statearr_46883_46894[(2)] = null);

(statearr_46883_46894[(1)] = (2));


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
});
return (function() {
var figwheel$client$heads_up_plugin_$_state_machine__26706__auto__ = null;
var figwheel$client$heads_up_plugin_$_state_machine__26706__auto____0 = (function (){
var statearr_46884 = [null,null,null,null,null,null,null,null,null];
(statearr_46884[(0)] = figwheel$client$heads_up_plugin_$_state_machine__26706__auto__);

(statearr_46884[(1)] = (1));

return statearr_46884;
});
var figwheel$client$heads_up_plugin_$_state_machine__26706__auto____1 = (function (state_46874){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_46874);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e46885){if((e46885 instanceof Object)){
var ex__26709__auto__ = e46885;
var statearr_46886_46895 = state_46874;
(statearr_46886_46895[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_46874);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e46885;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__46896 = state_46874;
state_46874 = G__46896;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
figwheel$client$heads_up_plugin_$_state_machine__26706__auto__ = function(state_46874){
switch(arguments.length){
case 0:
return figwheel$client$heads_up_plugin_$_state_machine__26706__auto____0.call(this);
case 1:
return figwheel$client$heads_up_plugin_$_state_machine__26706__auto____1.call(this,state_46874);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
figwheel$client$heads_up_plugin_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = figwheel$client$heads_up_plugin_$_state_machine__26706__auto____0;
figwheel$client$heads_up_plugin_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = figwheel$client$heads_up_plugin_$_state_machine__26706__auto____1;
return figwheel$client$heads_up_plugin_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_46887 = f__26801__auto__.call(null);
(statearr_46887[(6)] = c__26800__auto___46888);

return statearr_46887;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


figwheel.client.heads_up.ensure_container.call(null);

return (function (msg_hist){
cljs.core.async.put_BANG_.call(null,ch,msg_hist);

return msg_hist;
});
});
figwheel.client.enforce_project_plugin = (function figwheel$client$enforce_project_plugin(opts){
return (function (msg_hist){
if(((1) < cljs.core.count.call(null,cljs.core.set.call(null,cljs.core.keep.call(null,new cljs.core.Keyword(null,"project-id","project-id",206449307),cljs.core.take.call(null,(5),msg_hist)))))){
figwheel.client.socket.close_BANG_.call(null);

console.error("Figwheel: message received from different project. Shutting socket down.");

if(cljs.core.truth_(new cljs.core.Keyword(null,"heads-up-display","heads-up-display",-896577202).cljs$core$IFn$_invoke$arity$1(opts))){
var c__26800__auto__ = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_46902){
var state_val_46903 = (state_46902[(1)]);
if((state_val_46903 === (1))){
var inst_46897 = cljs.core.async.timeout.call(null,(3000));
var state_46902__$1 = state_46902;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46902__$1,(2),inst_46897);
} else {
if((state_val_46903 === (2))){
var inst_46899 = (state_46902[(2)]);
var inst_46900 = figwheel.client.heads_up.display_system_warning.call(null,"Connection from different project","Shutting connection down!!!!!");
var state_46902__$1 = (function (){var statearr_46904 = state_46902;
(statearr_46904[(7)] = inst_46899);

return statearr_46904;
})();
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_46902__$1,inst_46900);
} else {
return null;
}
}
});
return (function() {
var figwheel$client$enforce_project_plugin_$_state_machine__26706__auto__ = null;
var figwheel$client$enforce_project_plugin_$_state_machine__26706__auto____0 = (function (){
var statearr_46905 = [null,null,null,null,null,null,null,null];
(statearr_46905[(0)] = figwheel$client$enforce_project_plugin_$_state_machine__26706__auto__);

(statearr_46905[(1)] = (1));

return statearr_46905;
});
var figwheel$client$enforce_project_plugin_$_state_machine__26706__auto____1 = (function (state_46902){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_46902);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e46906){if((e46906 instanceof Object)){
var ex__26709__auto__ = e46906;
var statearr_46907_46909 = state_46902;
(statearr_46907_46909[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_46902);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e46906;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__46910 = state_46902;
state_46902 = G__46910;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
figwheel$client$enforce_project_plugin_$_state_machine__26706__auto__ = function(state_46902){
switch(arguments.length){
case 0:
return figwheel$client$enforce_project_plugin_$_state_machine__26706__auto____0.call(this);
case 1:
return figwheel$client$enforce_project_plugin_$_state_machine__26706__auto____1.call(this,state_46902);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
figwheel$client$enforce_project_plugin_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = figwheel$client$enforce_project_plugin_$_state_machine__26706__auto____0;
figwheel$client$enforce_project_plugin_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = figwheel$client$enforce_project_plugin_$_state_machine__26706__auto____1;
return figwheel$client$enforce_project_plugin_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_46908 = f__26801__auto__.call(null);
(statearr_46908[(6)] = c__26800__auto__);

return statearr_46908;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));

return c__26800__auto__;
} else {
return null;
}
} else {
return null;
}
});
});
figwheel.client.enforce_figwheel_version_plugin = (function figwheel$client$enforce_figwheel_version_plugin(opts){
return (function (msg_hist){
var temp__5720__auto__ = new cljs.core.Keyword(null,"figwheel-version","figwheel-version",1409553832).cljs$core$IFn$_invoke$arity$1(cljs.core.first.call(null,msg_hist));
if(cljs.core.truth_(temp__5720__auto__)){
var figwheel_version = temp__5720__auto__;
if(cljs.core.not_EQ_.call(null,figwheel_version,figwheel.client._figwheel_version_)){
figwheel.client.socket.close_BANG_.call(null);

console.error("Figwheel: message received from different version of Figwheel.");

if(cljs.core.truth_(new cljs.core.Keyword(null,"heads-up-display","heads-up-display",-896577202).cljs$core$IFn$_invoke$arity$1(opts))){
var c__26800__auto__ = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_46917){
var state_val_46918 = (state_46917[(1)]);
if((state_val_46918 === (1))){
var inst_46911 = cljs.core.async.timeout.call(null,(2000));
var state_46917__$1 = state_46917;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_46917__$1,(2),inst_46911);
} else {
if((state_val_46918 === (2))){
var inst_46913 = (state_46917[(2)]);
var inst_46914 = ["Figwheel Client Version <strong>",figwheel.client._figwheel_version_,"</strong> is not equal to ","Figwheel Sidecar Version <strong>",cljs.core.str.cljs$core$IFn$_invoke$arity$1(figwheel_version),"</strong>",".  Shutting down Websocket Connection!","<h4>To fix try:</h4>","<ol><li>Reload this page and make sure you are not getting a cached version of the client.</li>","<li>You may have to clean (delete compiled assets) and rebuild to make sure that the new client code is being used.</li>","<li>Also, make sure you have consistent Figwheel dependencies.</li></ol>"].join('');
var inst_46915 = figwheel.client.heads_up.display_system_warning.call(null,"Figwheel Client and Server have different versions!!",inst_46914);
var state_46917__$1 = (function (){var statearr_46919 = state_46917;
(statearr_46919[(7)] = inst_46913);

return statearr_46919;
})();
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_46917__$1,inst_46915);
} else {
return null;
}
}
});
return (function() {
var figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto__ = null;
var figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto____0 = (function (){
var statearr_46920 = [null,null,null,null,null,null,null,null];
(statearr_46920[(0)] = figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto__);

(statearr_46920[(1)] = (1));

return statearr_46920;
});
var figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto____1 = (function (state_46917){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_46917);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e46921){if((e46921 instanceof Object)){
var ex__26709__auto__ = e46921;
var statearr_46922_46924 = state_46917;
(statearr_46922_46924[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_46917);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e46921;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__46925 = state_46917;
state_46917 = G__46925;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto__ = function(state_46917){
switch(arguments.length){
case 0:
return figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto____0.call(this);
case 1:
return figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto____1.call(this,state_46917);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto____0;
figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto____1;
return figwheel$client$enforce_figwheel_version_plugin_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_46923 = f__26801__auto__.call(null);
(statearr_46923[(6)] = c__26800__auto__);

return statearr_46923;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));

return c__26800__auto__;
} else {
return null;
}
} else {
return null;
}
} else {
return null;
}
});
});
figwheel.client.default_on_jsload = cljs.core.identity;
figwheel.client.file_line_column = (function figwheel$client$file_line_column(p__46926){
var map__46927 = p__46926;
var map__46927__$1 = (((((!((map__46927 == null))))?(((((map__46927.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46927.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46927):map__46927);
var file = cljs.core.get.call(null,map__46927__$1,new cljs.core.Keyword(null,"file","file",-1269645878));
var line = cljs.core.get.call(null,map__46927__$1,new cljs.core.Keyword(null,"line","line",212345235));
var column = cljs.core.get.call(null,map__46927__$1,new cljs.core.Keyword(null,"column","column",2078222095));
var G__46929 = "";
var G__46929__$1 = (cljs.core.truth_(file)?[G__46929,"file ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(file)].join(''):G__46929);
var G__46929__$2 = (cljs.core.truth_(line)?[G__46929__$1," at line ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(line)].join(''):G__46929__$1);
if(cljs.core.truth_((function (){var and__4115__auto__ = line;
if(cljs.core.truth_(and__4115__auto__)){
return column;
} else {
return and__4115__auto__;
}
})())){
return [G__46929__$2,", column ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(column)].join('');
} else {
return G__46929__$2;
}
});
figwheel.client.default_on_compile_fail = (function figwheel$client$default_on_compile_fail(p__46930){
var map__46931 = p__46930;
var map__46931__$1 = (((((!((map__46931 == null))))?(((((map__46931.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46931.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46931):map__46931);
var ed = map__46931__$1;
var exception_data = cljs.core.get.call(null,map__46931__$1,new cljs.core.Keyword(null,"exception-data","exception-data",-512474886));
var cause = cljs.core.get.call(null,map__46931__$1,new cljs.core.Keyword(null,"cause","cause",231901252));
var message_46934 = (function (){var G__46933 = cljs.core.apply.call(null,cljs.core.str,"Figwheel: Compile Exception ",figwheel.client.format_messages.call(null,exception_data));
if(cljs.core.truth_(new cljs.core.Keyword(null,"file","file",-1269645878).cljs$core$IFn$_invoke$arity$1(exception_data))){
return [cljs.core.str.cljs$core$IFn$_invoke$arity$1(G__46933)," Error on ",figwheel.client.file_line_column.call(null,exception_data)].join('');
} else {
return G__46933;
}
})();
figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"warn","warn",-436710552),message_46934);

return ed;
});
figwheel.client.default_on_compile_warning = (function figwheel$client$default_on_compile_warning(p__46935){
var map__46936 = p__46935;
var map__46936__$1 = (((((!((map__46936 == null))))?(((((map__46936.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46936.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46936):map__46936);
var w = map__46936__$1;
var message = cljs.core.get.call(null,map__46936__$1,new cljs.core.Keyword(null,"message","message",-406056002));
figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"warn","warn",-436710552),["Figwheel: Compile Warning - ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(new cljs.core.Keyword(null,"message","message",-406056002).cljs$core$IFn$_invoke$arity$1(message))," in ",figwheel.client.file_line_column.call(null,message)].join(''));

return w;
});
figwheel.client.default_before_load = (function figwheel$client$default_before_load(files){
figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"debug","debug",-1608172596),"Figwheel: notified of file changes");

return files;
});
figwheel.client.default_on_cssload = (function figwheel$client$default_on_cssload(files){
figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"debug","debug",-1608172596),"Figwheel: loaded CSS files");

figwheel.client.utils.log.call(null,new cljs.core.Keyword(null,"info","info",-317069002),cljs.core.pr_str.call(null,cljs.core.map.call(null,new cljs.core.Keyword(null,"file","file",-1269645878),files)));

return files;
});
if((typeof figwheel !== 'undefined') && (typeof figwheel.client !== 'undefined') && (typeof figwheel.client.config_defaults !== 'undefined')){
} else {
figwheel.client.config_defaults = cljs.core.PersistentHashMap.fromArrays([new cljs.core.Keyword(null,"on-compile-warning","on-compile-warning",-1195585947),new cljs.core.Keyword(null,"on-jsload","on-jsload",-395756602),new cljs.core.Keyword(null,"reload-dependents","reload-dependents",-956865430),new cljs.core.Keyword(null,"on-compile-fail","on-compile-fail",728013036),new cljs.core.Keyword(null,"debug","debug",-1608172596),new cljs.core.Keyword(null,"heads-up-display","heads-up-display",-896577202),new cljs.core.Keyword(null,"websocket-url","websocket-url",-490444938),new cljs.core.Keyword(null,"auto-jump-to-source-on-error","auto-jump-to-source-on-error",-960314920),new cljs.core.Keyword(null,"before-jsload","before-jsload",-847513128),new cljs.core.Keyword(null,"load-warninged-code","load-warninged-code",-2030345223),new cljs.core.Keyword(null,"eval-fn","eval-fn",-1111644294),new cljs.core.Keyword(null,"retry-count","retry-count",1936122875),new cljs.core.Keyword(null,"autoload","autoload",-354122500),new cljs.core.Keyword(null,"on-cssload","on-cssload",1825432318)],[new cljs.core.Var(function(){return figwheel.client.default_on_compile_warning;},new cljs.core.Symbol("figwheel.client","default-on-compile-warning","figwheel.client/default-on-compile-warning",584144208,null),cljs.core.PersistentHashMap.fromArrays([new cljs.core.Keyword(null,"ns","ns",441598760),new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"file","file",-1269645878),new cljs.core.Keyword(null,"end-column","end-column",1425389514),new cljs.core.Keyword(null,"column","column",2078222095),new cljs.core.Keyword(null,"line","line",212345235),new cljs.core.Keyword(null,"end-line","end-line",1837326455),new cljs.core.Keyword(null,"arglists","arglists",1661989754),new cljs.core.Keyword(null,"doc","doc",1913296891),new cljs.core.Keyword(null,"test","test",577538877)],[new cljs.core.Symbol(null,"figwheel.client","figwheel.client",-538710252,null),new cljs.core.Symbol(null,"default-on-compile-warning","default-on-compile-warning",-18911586,null),"resources/public/js/compiled/out/figwheel/client.cljs",33,1,362,362,cljs.core.list(new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"keys","keys",1068423698),new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"message","message",1234475525,null)], null),new cljs.core.Keyword(null,"as","as",1148689641),new cljs.core.Symbol(null,"w","w",1994700528,null)], null)], null)),null,(cljs.core.truth_(figwheel.client.default_on_compile_warning)?figwheel.client.default_on_compile_warning.cljs$lang$test:null)])),figwheel.client.default_on_jsload,true,new cljs.core.Var(function(){return figwheel.client.default_on_compile_fail;},new cljs.core.Symbol("figwheel.client","default-on-compile-fail","figwheel.client/default-on-compile-fail",1384826337,null),cljs.core.PersistentHashMap.fromArrays([new cljs.core.Keyword(null,"ns","ns",441598760),new cljs.core.Keyword(null,"name","name",1843675177),new cljs.core.Keyword(null,"file","file",-1269645878),new cljs.core.Keyword(null,"end-column","end-column",1425389514),new cljs.core.Keyword(null,"column","column",2078222095),new cljs.core.Keyword(null,"line","line",212345235),new cljs.core.Keyword(null,"end-line","end-line",1837326455),new cljs.core.Keyword(null,"arglists","arglists",1661989754),new cljs.core.Keyword(null,"doc","doc",1913296891),new cljs.core.Keyword(null,"test","test",577538877)],[new cljs.core.Symbol(null,"figwheel.client","figwheel.client",-538710252,null),new cljs.core.Symbol(null,"default-on-compile-fail","default-on-compile-fail",-158814813,null),"resources/public/js/compiled/out/figwheel/client.cljs",30,1,355,355,cljs.core.list(new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"keys","keys",1068423698),new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"exception-data","exception-data",1128056641,null),new cljs.core.Symbol(null,"cause","cause",1872432779,null)], null),new cljs.core.Keyword(null,"as","as",1148689641),new cljs.core.Symbol(null,"ed","ed",2076825751,null)], null)], null)),null,(cljs.core.truth_(figwheel.client.default_on_compile_fail)?figwheel.client.default_on_compile_fail.cljs$lang$test:null)])),false,true,["ws://",cljs.core.str.cljs$core$IFn$_invoke$arity$1(((figwheel.client.utils.html_env_QMARK_.call(null))?location.host:"localhost:3449")),"/figwheel-ws"].join(''),false,figwheel.client.default_before_load,false,false,(100),true,figwheel.client.default_on_cssload]);
}
figwheel.client.handle_deprecated_jsload_callback = (function figwheel$client$handle_deprecated_jsload_callback(config){
if(cljs.core.truth_(new cljs.core.Keyword(null,"jsload-callback","jsload-callback",-1949628369).cljs$core$IFn$_invoke$arity$1(config))){
return cljs.core.dissoc.call(null,cljs.core.assoc.call(null,config,new cljs.core.Keyword(null,"on-jsload","on-jsload",-395756602),new cljs.core.Keyword(null,"jsload-callback","jsload-callback",-1949628369).cljs$core$IFn$_invoke$arity$1(config)),new cljs.core.Keyword(null,"jsload-callback","jsload-callback",-1949628369));
} else {
return config;
}
});
figwheel.client.fill_url_template = (function figwheel$client$fill_url_template(config){
if(figwheel.client.utils.html_env_QMARK_.call(null)){
return cljs.core.update_in.call(null,config,new cljs.core.PersistentVector(null, 1, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"websocket-url","websocket-url",-490444938)], null),(function (x){
return clojure.string.replace.call(null,clojure.string.replace.call(null,x,"[[client-hostname]]",location.hostname),"[[client-port]]",location.port);
}));
} else {
return config;
}
});
figwheel.client.base_plugins = (function figwheel$client$base_plugins(system_options){
var base = new cljs.core.PersistentArrayMap(null, 6, [new cljs.core.Keyword(null,"enforce-project-plugin","enforce-project-plugin",959402899),figwheel.client.enforce_project_plugin,new cljs.core.Keyword(null,"enforce-figwheel-version-plugin","enforce-figwheel-version-plugin",-1916185220),figwheel.client.enforce_figwheel_version_plugin,new cljs.core.Keyword(null,"file-reloader-plugin","file-reloader-plugin",-1792964733),figwheel.client.file_reloader_plugin,new cljs.core.Keyword(null,"comp-fail-warning-plugin","comp-fail-warning-plugin",634311),figwheel.client.compile_fail_warning_plugin,new cljs.core.Keyword(null,"css-reloader-plugin","css-reloader-plugin",2002032904),figwheel.client.css_reloader_plugin,new cljs.core.Keyword(null,"repl-plugin","repl-plugin",-1138952371),figwheel.client.repl_plugin], null);
var base__$1 = (((!(figwheel.client.utils.html_env_QMARK_.call(null))))?cljs.core.select_keys.call(null,base,new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"file-reloader-plugin","file-reloader-plugin",-1792964733),new cljs.core.Keyword(null,"comp-fail-warning-plugin","comp-fail-warning-plugin",634311),new cljs.core.Keyword(null,"repl-plugin","repl-plugin",-1138952371)], null)):base);
var base__$2 = ((new cljs.core.Keyword(null,"autoload","autoload",-354122500).cljs$core$IFn$_invoke$arity$1(system_options) === false)?cljs.core.dissoc.call(null,base__$1,new cljs.core.Keyword(null,"file-reloader-plugin","file-reloader-plugin",-1792964733)):base__$1);
if(cljs.core.truth_((function (){var and__4115__auto__ = new cljs.core.Keyword(null,"heads-up-display","heads-up-display",-896577202).cljs$core$IFn$_invoke$arity$1(system_options);
if(cljs.core.truth_(and__4115__auto__)){
return figwheel.client.utils.html_env_QMARK_.call(null);
} else {
return and__4115__auto__;
}
})())){
return cljs.core.assoc.call(null,base__$2,new cljs.core.Keyword(null,"heads-up-display-plugin","heads-up-display-plugin",1745207501),figwheel.client.heads_up_plugin);
} else {
return base__$2;
}
});
figwheel.client.add_message_watch = (function figwheel$client$add_message_watch(key,callback){
return cljs.core.add_watch.call(null,figwheel.client.socket.message_history_atom,key,(function (_,___$1,___$2,msg_hist){
return callback.call(null,cljs.core.first.call(null,msg_hist));
}));
});
figwheel.client.add_json_message_watch = (function figwheel$client$add_json_message_watch(key,callback){
return figwheel.client.add_message_watch.call(null,key,cljs.core.comp.call(null,callback,cljs.core.clj__GT_js));
});
goog.exportSymbol('figwheel.client.add_json_message_watch', figwheel.client.add_json_message_watch);
figwheel.client.add_plugins = (function figwheel$client$add_plugins(plugins,system_options){
var seq__46938 = cljs.core.seq.call(null,plugins);
var chunk__46939 = null;
var count__46940 = (0);
var i__46941 = (0);
while(true){
if((i__46941 < count__46940)){
var vec__46948 = cljs.core._nth.call(null,chunk__46939,i__46941);
var k = cljs.core.nth.call(null,vec__46948,(0),null);
var plugin = cljs.core.nth.call(null,vec__46948,(1),null);
if(cljs.core.truth_(plugin)){
var pl_46954 = plugin.call(null,system_options);
cljs.core.add_watch.call(null,figwheel.client.socket.message_history_atom,k,((function (seq__46938,chunk__46939,count__46940,i__46941,pl_46954,vec__46948,k,plugin){
return (function (_,___$1,___$2,msg_hist){
return pl_46954.call(null,msg_hist);
});})(seq__46938,chunk__46939,count__46940,i__46941,pl_46954,vec__46948,k,plugin))
);
} else {
}


var G__46955 = seq__46938;
var G__46956 = chunk__46939;
var G__46957 = count__46940;
var G__46958 = (i__46941 + (1));
seq__46938 = G__46955;
chunk__46939 = G__46956;
count__46940 = G__46957;
i__46941 = G__46958;
continue;
} else {
var temp__5720__auto__ = cljs.core.seq.call(null,seq__46938);
if(temp__5720__auto__){
var seq__46938__$1 = temp__5720__auto__;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__46938__$1)){
var c__4556__auto__ = cljs.core.chunk_first.call(null,seq__46938__$1);
var G__46959 = cljs.core.chunk_rest.call(null,seq__46938__$1);
var G__46960 = c__4556__auto__;
var G__46961 = cljs.core.count.call(null,c__4556__auto__);
var G__46962 = (0);
seq__46938 = G__46959;
chunk__46939 = G__46960;
count__46940 = G__46961;
i__46941 = G__46962;
continue;
} else {
var vec__46951 = cljs.core.first.call(null,seq__46938__$1);
var k = cljs.core.nth.call(null,vec__46951,(0),null);
var plugin = cljs.core.nth.call(null,vec__46951,(1),null);
if(cljs.core.truth_(plugin)){
var pl_46963 = plugin.call(null,system_options);
cljs.core.add_watch.call(null,figwheel.client.socket.message_history_atom,k,((function (seq__46938,chunk__46939,count__46940,i__46941,pl_46963,vec__46951,k,plugin,seq__46938__$1,temp__5720__auto__){
return (function (_,___$1,___$2,msg_hist){
return pl_46963.call(null,msg_hist);
});})(seq__46938,chunk__46939,count__46940,i__46941,pl_46963,vec__46951,k,plugin,seq__46938__$1,temp__5720__auto__))
);
} else {
}


var G__46964 = cljs.core.next.call(null,seq__46938__$1);
var G__46965 = null;
var G__46966 = (0);
var G__46967 = (0);
seq__46938 = G__46964;
chunk__46939 = G__46965;
count__46940 = G__46966;
i__46941 = G__46967;
continue;
}
} else {
return null;
}
}
break;
}
});
figwheel.client.start = (function figwheel$client$start(var_args){
var G__46969 = arguments.length;
switch (G__46969) {
case 1:
return figwheel.client.start.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 0:
return figwheel.client.start.cljs$core$IFn$_invoke$arity$0();

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(figwheel.client.start.cljs$core$IFn$_invoke$arity$1 = (function (opts){
if((((!((goog.dependencies_ == null)))) || ((((!((goog.debugLoader_ == null)))) && ((!((goog.debugLoader_.dependencies_ == null)))))))){
if((typeof figwheel !== 'undefined') && (typeof figwheel.client !== 'undefined') && (typeof figwheel.client.__figwheel_start_once__ !== 'undefined')){
return null;
} else {
return (
figwheel.client.__figwheel_start_once__ = setTimeout((function (){
var plugins_SINGLEQUOTE_ = new cljs.core.Keyword(null,"plugins","plugins",1900073717).cljs$core$IFn$_invoke$arity$1(opts);
var merge_plugins = new cljs.core.Keyword(null,"merge-plugins","merge-plugins",-1193912370).cljs$core$IFn$_invoke$arity$1(opts);
var system_options = figwheel.client.fill_url_template.call(null,figwheel.client.handle_deprecated_jsload_callback.call(null,cljs.core.merge.call(null,figwheel.client.config_defaults,cljs.core.dissoc.call(null,opts,new cljs.core.Keyword(null,"plugins","plugins",1900073717),new cljs.core.Keyword(null,"merge-plugins","merge-plugins",-1193912370)))));
var plugins = (cljs.core.truth_(plugins_SINGLEQUOTE_)?plugins_SINGLEQUOTE_:cljs.core.merge.call(null,figwheel.client.base_plugins.call(null,system_options),merge_plugins));
(figwheel.client.utils._STAR_print_debug_STAR_ = new cljs.core.Keyword(null,"debug","debug",-1608172596).cljs$core$IFn$_invoke$arity$1(opts));

figwheel.client.enable_repl_print_BANG_.call(null);

figwheel.client.add_plugins.call(null,plugins,system_options);

figwheel.client.file_reloading.patch_goog_base.call(null);

var seq__46970_46975 = cljs.core.seq.call(null,new cljs.core.Keyword(null,"initial-messages","initial-messages",2057377771).cljs$core$IFn$_invoke$arity$1(system_options));
var chunk__46971_46976 = null;
var count__46972_46977 = (0);
var i__46973_46978 = (0);
while(true){
if((i__46973_46978 < count__46972_46977)){
var msg_46979 = cljs.core._nth.call(null,chunk__46971_46976,i__46973_46978);
figwheel.client.socket.handle_incoming_message.call(null,msg_46979);


var G__46980 = seq__46970_46975;
var G__46981 = chunk__46971_46976;
var G__46982 = count__46972_46977;
var G__46983 = (i__46973_46978 + (1));
seq__46970_46975 = G__46980;
chunk__46971_46976 = G__46981;
count__46972_46977 = G__46982;
i__46973_46978 = G__46983;
continue;
} else {
var temp__5720__auto___46984 = cljs.core.seq.call(null,seq__46970_46975);
if(temp__5720__auto___46984){
var seq__46970_46985__$1 = temp__5720__auto___46984;
if(cljs.core.chunked_seq_QMARK_.call(null,seq__46970_46985__$1)){
var c__4556__auto___46986 = cljs.core.chunk_first.call(null,seq__46970_46985__$1);
var G__46987 = cljs.core.chunk_rest.call(null,seq__46970_46985__$1);
var G__46988 = c__4556__auto___46986;
var G__46989 = cljs.core.count.call(null,c__4556__auto___46986);
var G__46990 = (0);
seq__46970_46975 = G__46987;
chunk__46971_46976 = G__46988;
count__46972_46977 = G__46989;
i__46973_46978 = G__46990;
continue;
} else {
var msg_46991 = cljs.core.first.call(null,seq__46970_46985__$1);
figwheel.client.socket.handle_incoming_message.call(null,msg_46991);


var G__46992 = cljs.core.next.call(null,seq__46970_46985__$1);
var G__46993 = null;
var G__46994 = (0);
var G__46995 = (0);
seq__46970_46975 = G__46992;
chunk__46971_46976 = G__46993;
count__46972_46977 = G__46994;
i__46973_46978 = G__46995;
continue;
}
} else {
}
}
break;
}

return figwheel.client.socket.open.call(null,system_options);
})))
;
}
} else {
return null;
}
}));

(figwheel.client.start.cljs$core$IFn$_invoke$arity$0 = (function (){
return figwheel.client.start.call(null,cljs.core.PersistentArrayMap.EMPTY);
}));

(figwheel.client.start.cljs$lang$maxFixedArity = 1);

figwheel.client.watch_and_reload_with_opts = figwheel.client.start;
figwheel.client.watch_and_reload = (function figwheel$client$watch_and_reload(var_args){
var args__4742__auto__ = [];
var len__4736__auto___47000 = arguments.length;
var i__4737__auto___47001 = (0);
while(true){
if((i__4737__auto___47001 < len__4736__auto___47000)){
args__4742__auto__.push((arguments[i__4737__auto___47001]));

var G__47002 = (i__4737__auto___47001 + (1));
i__4737__auto___47001 = G__47002;
continue;
} else {
}
break;
}

var argseq__4743__auto__ = ((((0) < args__4742__auto__.length))?(new cljs.core.IndexedSeq(args__4742__auto__.slice((0)),(0),null)):null);
return figwheel.client.watch_and_reload.cljs$core$IFn$_invoke$arity$variadic(argseq__4743__auto__);
});

(figwheel.client.watch_and_reload.cljs$core$IFn$_invoke$arity$variadic = (function (p__46997){
var map__46998 = p__46997;
var map__46998__$1 = (((((!((map__46998 == null))))?(((((map__46998.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__46998.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__46998):map__46998);
var opts = map__46998__$1;
return figwheel.client.start.call(null,opts);
}));

(figwheel.client.watch_and_reload.cljs$lang$maxFixedArity = (0));

/** @this {Function} */
(figwheel.client.watch_and_reload.cljs$lang$applyTo = (function (seq46996){
var self__4724__auto__ = this;
return self__4724__auto__.cljs$core$IFn$_invoke$arity$variadic(cljs.core.seq.call(null,seq46996));
}));

figwheel.client.fetch_data_from_env = (function figwheel$client$fetch_data_from_env(){
try{return cljs.reader.read_string.call(null,goog.object.get(window,"FIGWHEEL_CLIENT_CONFIGURATION"));
}catch (e47003){if((e47003 instanceof Error)){
var e = e47003;
cljs.core._STAR_print_err_fn_STAR_.call(null,"Unable to load FIGWHEEL_CLIENT_CONFIGURATION from the environment");

return new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"autoload","autoload",-354122500),false], null);
} else {
throw e47003;

}
}});
figwheel.client.console_intro_message = "Figwheel has compiled a temporary helper application to your :output-file.\n\nThe code currently in your configured output file does not\nrepresent the code that you are trying to compile.\n\nThis temporary application is intended to help you continue to get\nfeedback from Figwheel until the build you are working on compiles\ncorrectly.\n\nWhen your ClojureScript source code compiles correctly this helper\napplication will auto-reload and pick up your freshly compiled\nClojureScript program.";
figwheel.client.bad_compile_helper_app = (function figwheel$client$bad_compile_helper_app(){
cljs.core.enable_console_print_BANG_.call(null);

var config = figwheel.client.fetch_data_from_env.call(null);
cljs.core.println.call(null,figwheel.client.console_intro_message);

figwheel.client.heads_up.bad_compile_screen.call(null);

if(cljs.core.truth_(goog.dependencies_)){
} else {
(goog.dependencies_ = true);
}

figwheel.client.start.call(null,config);

return figwheel.client.add_message_watch.call(null,new cljs.core.Keyword(null,"listen-for-successful-compile","listen-for-successful-compile",-995277603),(function (p__47004){
var map__47005 = p__47004;
var map__47005__$1 = (((((!((map__47005 == null))))?(((((map__47005.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__47005.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__47005):map__47005);
var msg_name = cljs.core.get.call(null,map__47005__$1,new cljs.core.Keyword(null,"msg-name","msg-name",-353709863));
if(cljs.core._EQ_.call(null,msg_name,new cljs.core.Keyword(null,"files-changed","files-changed",-1418200563))){
return (location.href = location.href);
} else {
return null;
}
}));
});

//# sourceMappingURL=client.js.map?rel=1654487553776
