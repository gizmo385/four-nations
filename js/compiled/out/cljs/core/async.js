// Compiled by ClojureScript 1.10.773 {}
goog.provide('cljs.core.async');
goog.require('cljs.core');
goog.require('cljs.core.async.impl.protocols');
goog.require('cljs.core.async.impl.channels');
goog.require('cljs.core.async.impl.buffers');
goog.require('cljs.core.async.impl.timers');
goog.require('cljs.core.async.impl.dispatch');
goog.require('cljs.core.async.impl.ioc_helpers');
goog.require('goog.array');
cljs.core.async.fn_handler = (function cljs$core$async$fn_handler(var_args){
var G__42674 = arguments.length;
switch (G__42674) {
case 1:
return cljs.core.async.fn_handler.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 2:
return cljs.core.async.fn_handler.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.fn_handler.cljs$core$IFn$_invoke$arity$1 = (function (f){
return cljs.core.async.fn_handler.call(null,f,true);
}));

(cljs.core.async.fn_handler.cljs$core$IFn$_invoke$arity$2 = (function (f,blockable){
if((typeof cljs !== 'undefined') && (typeof cljs.core !== 'undefined') && (typeof cljs.core.async !== 'undefined') && (typeof cljs.core.async.t_cljs$core$async42675 !== 'undefined')){
} else {

/**
* @constructor
 * @implements {cljs.core.async.impl.protocols.Handler}
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.IWithMeta}
*/
cljs.core.async.t_cljs$core$async42675 = (function (f,blockable,meta42676){
this.f = f;
this.blockable = blockable;
this.meta42676 = meta42676;
this.cljs$lang$protocol_mask$partition0$ = 393216;
this.cljs$lang$protocol_mask$partition1$ = 0;
});
(cljs.core.async.t_cljs$core$async42675.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (_42677,meta42676__$1){
var self__ = this;
var _42677__$1 = this;
return (new cljs.core.async.t_cljs$core$async42675(self__.f,self__.blockable,meta42676__$1));
}));

(cljs.core.async.t_cljs$core$async42675.prototype.cljs$core$IMeta$_meta$arity$1 = (function (_42677){
var self__ = this;
var _42677__$1 = this;
return self__.meta42676;
}));

(cljs.core.async.t_cljs$core$async42675.prototype.cljs$core$async$impl$protocols$Handler$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async42675.prototype.cljs$core$async$impl$protocols$Handler$active_QMARK_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return true;
}));

(cljs.core.async.t_cljs$core$async42675.prototype.cljs$core$async$impl$protocols$Handler$blockable_QMARK_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return self__.blockable;
}));

(cljs.core.async.t_cljs$core$async42675.prototype.cljs$core$async$impl$protocols$Handler$commit$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return self__.f;
}));

(cljs.core.async.t_cljs$core$async42675.getBasis = (function (){
return new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"f","f",43394975,null),new cljs.core.Symbol(null,"blockable","blockable",-28395259,null),new cljs.core.Symbol(null,"meta42676","meta42676",967778844,null)], null);
}));

(cljs.core.async.t_cljs$core$async42675.cljs$lang$type = true);

(cljs.core.async.t_cljs$core$async42675.cljs$lang$ctorStr = "cljs.core.async/t_cljs$core$async42675");

(cljs.core.async.t_cljs$core$async42675.cljs$lang$ctorPrWriter = (function (this__4369__auto__,writer__4370__auto__,opt__4371__auto__){
return cljs.core._write.call(null,writer__4370__auto__,"cljs.core.async/t_cljs$core$async42675");
}));

/**
 * Positional factory function for cljs.core.async/t_cljs$core$async42675.
 */
cljs.core.async.__GT_t_cljs$core$async42675 = (function cljs$core$async$__GT_t_cljs$core$async42675(f__$1,blockable__$1,meta42676){
return (new cljs.core.async.t_cljs$core$async42675(f__$1,blockable__$1,meta42676));
});

}

return (new cljs.core.async.t_cljs$core$async42675(f,blockable,cljs.core.PersistentArrayMap.EMPTY));
}));

(cljs.core.async.fn_handler.cljs$lang$maxFixedArity = 2);

/**
 * Returns a fixed buffer of size n. When full, puts will block/park.
 */
cljs.core.async.buffer = (function cljs$core$async$buffer(n){
return cljs.core.async.impl.buffers.fixed_buffer.call(null,n);
});
/**
 * Returns a buffer of size n. When full, puts will complete but
 *   val will be dropped (no transfer).
 */
cljs.core.async.dropping_buffer = (function cljs$core$async$dropping_buffer(n){
return cljs.core.async.impl.buffers.dropping_buffer.call(null,n);
});
/**
 * Returns a buffer of size n. When full, puts will complete, and be
 *   buffered, but oldest elements in buffer will be dropped (not
 *   transferred).
 */
cljs.core.async.sliding_buffer = (function cljs$core$async$sliding_buffer(n){
return cljs.core.async.impl.buffers.sliding_buffer.call(null,n);
});
/**
 * Returns true if a channel created with buff will never block. That is to say,
 * puts into this buffer will never cause the buffer to be full. 
 */
cljs.core.async.unblocking_buffer_QMARK_ = (function cljs$core$async$unblocking_buffer_QMARK_(buff){
if((!((buff == null)))){
if(((false) || ((cljs.core.PROTOCOL_SENTINEL === buff.cljs$core$async$impl$protocols$UnblockingBuffer$)))){
return true;
} else {
if((!buff.cljs$lang$protocol_mask$partition$)){
return cljs.core.native_satisfies_QMARK_.call(null,cljs.core.async.impl.protocols.UnblockingBuffer,buff);
} else {
return false;
}
}
} else {
return cljs.core.native_satisfies_QMARK_.call(null,cljs.core.async.impl.protocols.UnblockingBuffer,buff);
}
});
/**
 * Creates a channel with an optional buffer, an optional transducer (like (map f),
 *   (filter p) etc or a composition thereof), and an optional exception handler.
 *   If buf-or-n is a number, will create and use a fixed buffer of that size. If a
 *   transducer is supplied a buffer must be specified. ex-handler must be a
 *   fn of one argument - if an exception occurs during transformation it will be called
 *   with the thrown value as an argument, and any non-nil return value will be placed
 *   in the channel.
 */
cljs.core.async.chan = (function cljs$core$async$chan(var_args){
var G__42681 = arguments.length;
switch (G__42681) {
case 0:
return cljs.core.async.chan.cljs$core$IFn$_invoke$arity$0();

break;
case 1:
return cljs.core.async.chan.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 2:
return cljs.core.async.chan.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.chan.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.chan.cljs$core$IFn$_invoke$arity$0 = (function (){
return cljs.core.async.chan.call(null,null);
}));

(cljs.core.async.chan.cljs$core$IFn$_invoke$arity$1 = (function (buf_or_n){
return cljs.core.async.chan.call(null,buf_or_n,null,null);
}));

(cljs.core.async.chan.cljs$core$IFn$_invoke$arity$2 = (function (buf_or_n,xform){
return cljs.core.async.chan.call(null,buf_or_n,xform,null);
}));

(cljs.core.async.chan.cljs$core$IFn$_invoke$arity$3 = (function (buf_or_n,xform,ex_handler){
var buf_or_n__$1 = ((cljs.core._EQ_.call(null,buf_or_n,(0)))?null:buf_or_n);
if(cljs.core.truth_(xform)){
if(cljs.core.truth_(buf_or_n__$1)){
} else {
throw (new Error(["Assert failed: ","buffer must be supplied when transducer is","\n","buf-or-n"].join('')));
}
} else {
}

return cljs.core.async.impl.channels.chan.call(null,((typeof buf_or_n__$1 === 'number')?cljs.core.async.buffer.call(null,buf_or_n__$1):buf_or_n__$1),xform,ex_handler);
}));

(cljs.core.async.chan.cljs$lang$maxFixedArity = 3);

/**
 * Creates a promise channel with an optional transducer, and an optional
 *   exception-handler. A promise channel can take exactly one value that consumers
 *   will receive. Once full, puts complete but val is dropped (no transfer).
 *   Consumers will block until either a value is placed in the channel or the
 *   channel is closed. See chan for the semantics of xform and ex-handler.
 */
cljs.core.async.promise_chan = (function cljs$core$async$promise_chan(var_args){
var G__42684 = arguments.length;
switch (G__42684) {
case 0:
return cljs.core.async.promise_chan.cljs$core$IFn$_invoke$arity$0();

break;
case 1:
return cljs.core.async.promise_chan.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 2:
return cljs.core.async.promise_chan.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.promise_chan.cljs$core$IFn$_invoke$arity$0 = (function (){
return cljs.core.async.promise_chan.call(null,null);
}));

(cljs.core.async.promise_chan.cljs$core$IFn$_invoke$arity$1 = (function (xform){
return cljs.core.async.promise_chan.call(null,xform,null);
}));

(cljs.core.async.promise_chan.cljs$core$IFn$_invoke$arity$2 = (function (xform,ex_handler){
return cljs.core.async.chan.call(null,cljs.core.async.impl.buffers.promise_buffer.call(null),xform,ex_handler);
}));

(cljs.core.async.promise_chan.cljs$lang$maxFixedArity = 2);

/**
 * Returns a channel that will close after msecs
 */
cljs.core.async.timeout = (function cljs$core$async$timeout(msecs){
return cljs.core.async.impl.timers.timeout.call(null,msecs);
});
/**
 * takes a val from port. Must be called inside a (go ...) block. Will
 *   return nil if closed. Will park if nothing is available.
 *   Returns true unless port is already closed
 */
cljs.core.async._LT__BANG_ = (function cljs$core$async$_LT__BANG_(port){
throw (new Error("<! used not in (go ...) block"));
});
/**
 * Asynchronously takes a val from port, passing to fn1. Will pass nil
 * if closed. If on-caller? (default true) is true, and value is
 * immediately available, will call fn1 on calling thread.
 * Returns nil.
 */
cljs.core.async.take_BANG_ = (function cljs$core$async$take_BANG_(var_args){
var G__42687 = arguments.length;
switch (G__42687) {
case 2:
return cljs.core.async.take_BANG_.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.take_BANG_.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.take_BANG_.cljs$core$IFn$_invoke$arity$2 = (function (port,fn1){
return cljs.core.async.take_BANG_.call(null,port,fn1,true);
}));

(cljs.core.async.take_BANG_.cljs$core$IFn$_invoke$arity$3 = (function (port,fn1,on_caller_QMARK_){
var ret = cljs.core.async.impl.protocols.take_BANG_.call(null,port,cljs.core.async.fn_handler.call(null,fn1));
if(cljs.core.truth_(ret)){
var val_42689 = cljs.core.deref.call(null,ret);
if(cljs.core.truth_(on_caller_QMARK_)){
fn1.call(null,val_42689);
} else {
cljs.core.async.impl.dispatch.run.call(null,(function (){
return fn1.call(null,val_42689);
}));
}
} else {
}

return null;
}));

(cljs.core.async.take_BANG_.cljs$lang$maxFixedArity = 3);

cljs.core.async.nop = (function cljs$core$async$nop(_){
return null;
});
cljs.core.async.fhnop = cljs.core.async.fn_handler.call(null,cljs.core.async.nop);
/**
 * puts a val into port. nil values are not allowed. Must be called
 *   inside a (go ...) block. Will park if no buffer space is available.
 *   Returns true unless port is already closed.
 */
cljs.core.async._GT__BANG_ = (function cljs$core$async$_GT__BANG_(port,val){
throw (new Error(">! used not in (go ...) block"));
});
/**
 * Asynchronously puts a val into port, calling fn1 (if supplied) when
 * complete. nil values are not allowed. Will throw if closed. If
 * on-caller? (default true) is true, and the put is immediately
 * accepted, will call fn1 on calling thread.  Returns nil.
 */
cljs.core.async.put_BANG_ = (function cljs$core$async$put_BANG_(var_args){
var G__42691 = arguments.length;
switch (G__42691) {
case 2:
return cljs.core.async.put_BANG_.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.put_BANG_.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
case 4:
return cljs.core.async.put_BANG_.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.put_BANG_.cljs$core$IFn$_invoke$arity$2 = (function (port,val){
var temp__5718__auto__ = cljs.core.async.impl.protocols.put_BANG_.call(null,port,val,cljs.core.async.fhnop);
if(cljs.core.truth_(temp__5718__auto__)){
var ret = temp__5718__auto__;
return cljs.core.deref.call(null,ret);
} else {
return true;
}
}));

(cljs.core.async.put_BANG_.cljs$core$IFn$_invoke$arity$3 = (function (port,val,fn1){
return cljs.core.async.put_BANG_.call(null,port,val,fn1,true);
}));

(cljs.core.async.put_BANG_.cljs$core$IFn$_invoke$arity$4 = (function (port,val,fn1,on_caller_QMARK_){
var temp__5718__auto__ = cljs.core.async.impl.protocols.put_BANG_.call(null,port,val,cljs.core.async.fn_handler.call(null,fn1));
if(cljs.core.truth_(temp__5718__auto__)){
var retb = temp__5718__auto__;
var ret = cljs.core.deref.call(null,retb);
if(cljs.core.truth_(on_caller_QMARK_)){
fn1.call(null,ret);
} else {
cljs.core.async.impl.dispatch.run.call(null,(function (){
return fn1.call(null,ret);
}));
}

return ret;
} else {
return true;
}
}));

(cljs.core.async.put_BANG_.cljs$lang$maxFixedArity = 4);

cljs.core.async.close_BANG_ = (function cljs$core$async$close_BANG_(port){
return cljs.core.async.impl.protocols.close_BANG_.call(null,port);
});
cljs.core.async.random_array = (function cljs$core$async$random_array(n){
var a = (new Array(n));
var n__4613__auto___42693 = n;
var x_42694 = (0);
while(true){
if((x_42694 < n__4613__auto___42693)){
(a[x_42694] = x_42694);

var G__42695 = (x_42694 + (1));
x_42694 = G__42695;
continue;
} else {
}
break;
}

goog.array.shuffle(a);

return a;
});
cljs.core.async.alt_flag = (function cljs$core$async$alt_flag(){
var flag = cljs.core.atom.call(null,true);
if((typeof cljs !== 'undefined') && (typeof cljs.core !== 'undefined') && (typeof cljs.core.async !== 'undefined') && (typeof cljs.core.async.t_cljs$core$async42696 !== 'undefined')){
} else {

/**
* @constructor
 * @implements {cljs.core.async.impl.protocols.Handler}
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.IWithMeta}
*/
cljs.core.async.t_cljs$core$async42696 = (function (flag,meta42697){
this.flag = flag;
this.meta42697 = meta42697;
this.cljs$lang$protocol_mask$partition0$ = 393216;
this.cljs$lang$protocol_mask$partition1$ = 0;
});
(cljs.core.async.t_cljs$core$async42696.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (_42698,meta42697__$1){
var self__ = this;
var _42698__$1 = this;
return (new cljs.core.async.t_cljs$core$async42696(self__.flag,meta42697__$1));
}));

(cljs.core.async.t_cljs$core$async42696.prototype.cljs$core$IMeta$_meta$arity$1 = (function (_42698){
var self__ = this;
var _42698__$1 = this;
return self__.meta42697;
}));

(cljs.core.async.t_cljs$core$async42696.prototype.cljs$core$async$impl$protocols$Handler$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async42696.prototype.cljs$core$async$impl$protocols$Handler$active_QMARK_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return cljs.core.deref.call(null,self__.flag);
}));

(cljs.core.async.t_cljs$core$async42696.prototype.cljs$core$async$impl$protocols$Handler$blockable_QMARK_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return true;
}));

(cljs.core.async.t_cljs$core$async42696.prototype.cljs$core$async$impl$protocols$Handler$commit$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
cljs.core.reset_BANG_.call(null,self__.flag,null);

return true;
}));

(cljs.core.async.t_cljs$core$async42696.getBasis = (function (){
return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"flag","flag",-1565787888,null),new cljs.core.Symbol(null,"meta42697","meta42697",-1442902933,null)], null);
}));

(cljs.core.async.t_cljs$core$async42696.cljs$lang$type = true);

(cljs.core.async.t_cljs$core$async42696.cljs$lang$ctorStr = "cljs.core.async/t_cljs$core$async42696");

(cljs.core.async.t_cljs$core$async42696.cljs$lang$ctorPrWriter = (function (this__4369__auto__,writer__4370__auto__,opt__4371__auto__){
return cljs.core._write.call(null,writer__4370__auto__,"cljs.core.async/t_cljs$core$async42696");
}));

/**
 * Positional factory function for cljs.core.async/t_cljs$core$async42696.
 */
cljs.core.async.__GT_t_cljs$core$async42696 = (function cljs$core$async$alt_flag_$___GT_t_cljs$core$async42696(flag__$1,meta42697){
return (new cljs.core.async.t_cljs$core$async42696(flag__$1,meta42697));
});

}

return (new cljs.core.async.t_cljs$core$async42696(flag,cljs.core.PersistentArrayMap.EMPTY));
});
cljs.core.async.alt_handler = (function cljs$core$async$alt_handler(flag,cb){
if((typeof cljs !== 'undefined') && (typeof cljs.core !== 'undefined') && (typeof cljs.core.async !== 'undefined') && (typeof cljs.core.async.t_cljs$core$async42699 !== 'undefined')){
} else {

/**
* @constructor
 * @implements {cljs.core.async.impl.protocols.Handler}
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.IWithMeta}
*/
cljs.core.async.t_cljs$core$async42699 = (function (flag,cb,meta42700){
this.flag = flag;
this.cb = cb;
this.meta42700 = meta42700;
this.cljs$lang$protocol_mask$partition0$ = 393216;
this.cljs$lang$protocol_mask$partition1$ = 0;
});
(cljs.core.async.t_cljs$core$async42699.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (_42701,meta42700__$1){
var self__ = this;
var _42701__$1 = this;
return (new cljs.core.async.t_cljs$core$async42699(self__.flag,self__.cb,meta42700__$1));
}));

(cljs.core.async.t_cljs$core$async42699.prototype.cljs$core$IMeta$_meta$arity$1 = (function (_42701){
var self__ = this;
var _42701__$1 = this;
return self__.meta42700;
}));

(cljs.core.async.t_cljs$core$async42699.prototype.cljs$core$async$impl$protocols$Handler$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async42699.prototype.cljs$core$async$impl$protocols$Handler$active_QMARK_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return cljs.core.async.impl.protocols.active_QMARK_.call(null,self__.flag);
}));

(cljs.core.async.t_cljs$core$async42699.prototype.cljs$core$async$impl$protocols$Handler$blockable_QMARK_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return true;
}));

(cljs.core.async.t_cljs$core$async42699.prototype.cljs$core$async$impl$protocols$Handler$commit$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
cljs.core.async.impl.protocols.commit.call(null,self__.flag);

return self__.cb;
}));

(cljs.core.async.t_cljs$core$async42699.getBasis = (function (){
return new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"flag","flag",-1565787888,null),new cljs.core.Symbol(null,"cb","cb",-2064487928,null),new cljs.core.Symbol(null,"meta42700","meta42700",-816848827,null)], null);
}));

(cljs.core.async.t_cljs$core$async42699.cljs$lang$type = true);

(cljs.core.async.t_cljs$core$async42699.cljs$lang$ctorStr = "cljs.core.async/t_cljs$core$async42699");

(cljs.core.async.t_cljs$core$async42699.cljs$lang$ctorPrWriter = (function (this__4369__auto__,writer__4370__auto__,opt__4371__auto__){
return cljs.core._write.call(null,writer__4370__auto__,"cljs.core.async/t_cljs$core$async42699");
}));

/**
 * Positional factory function for cljs.core.async/t_cljs$core$async42699.
 */
cljs.core.async.__GT_t_cljs$core$async42699 = (function cljs$core$async$alt_handler_$___GT_t_cljs$core$async42699(flag__$1,cb__$1,meta42700){
return (new cljs.core.async.t_cljs$core$async42699(flag__$1,cb__$1,meta42700));
});

}

return (new cljs.core.async.t_cljs$core$async42699(flag,cb,cljs.core.PersistentArrayMap.EMPTY));
});
/**
 * returns derefable [val port] if immediate, nil if enqueued
 */
cljs.core.async.do_alts = (function cljs$core$async$do_alts(fret,ports,opts){
if((cljs.core.count.call(null,ports) > (0))){
} else {
throw (new Error(["Assert failed: ","alts must have at least one channel operation","\n","(pos? (count ports))"].join('')));
}

var flag = cljs.core.async.alt_flag.call(null);
var n = cljs.core.count.call(null,ports);
var idxs = cljs.core.async.random_array.call(null,n);
var priority = new cljs.core.Keyword(null,"priority","priority",1431093715).cljs$core$IFn$_invoke$arity$1(opts);
var ret = (function (){var i = (0);
while(true){
if((i < n)){
var idx = (cljs.core.truth_(priority)?i:(idxs[i]));
var port = cljs.core.nth.call(null,ports,idx);
var wport = ((cljs.core.vector_QMARK_.call(null,port))?port.call(null,(0)):null);
var vbox = (cljs.core.truth_(wport)?(function (){var val = port.call(null,(1));
return cljs.core.async.impl.protocols.put_BANG_.call(null,wport,val,cljs.core.async.alt_handler.call(null,flag,((function (i,val,idx,port,wport,flag,n,idxs,priority){
return (function (p1__42702_SHARP_){
return fret.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [p1__42702_SHARP_,wport], null));
});})(i,val,idx,port,wport,flag,n,idxs,priority))
));
})():cljs.core.async.impl.protocols.take_BANG_.call(null,port,cljs.core.async.alt_handler.call(null,flag,((function (i,idx,port,wport,flag,n,idxs,priority){
return (function (p1__42703_SHARP_){
return fret.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [p1__42703_SHARP_,port], null));
});})(i,idx,port,wport,flag,n,idxs,priority))
)));
if(cljs.core.truth_(vbox)){
return cljs.core.async.impl.channels.box.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [cljs.core.deref.call(null,vbox),(function (){var or__4126__auto__ = wport;
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return port;
}
})()], null));
} else {
var G__42704 = (i + (1));
i = G__42704;
continue;
}
} else {
return null;
}
break;
}
})();
var or__4126__auto__ = ret;
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
if(cljs.core.contains_QMARK_.call(null,opts,new cljs.core.Keyword(null,"default","default",-1987822328))){
var temp__5720__auto__ = (function (){var and__4115__auto__ = cljs.core.async.impl.protocols.active_QMARK_.call(null,flag);
if(cljs.core.truth_(and__4115__auto__)){
return cljs.core.async.impl.protocols.commit.call(null,flag);
} else {
return and__4115__auto__;
}
})();
if(cljs.core.truth_(temp__5720__auto__)){
var got = temp__5720__auto__;
return cljs.core.async.impl.channels.box.call(null,new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Keyword(null,"default","default",-1987822328).cljs$core$IFn$_invoke$arity$1(opts),new cljs.core.Keyword(null,"default","default",-1987822328)], null));
} else {
return null;
}
} else {
return null;
}
}
});
/**
 * Completes at most one of several channel operations. Must be called
 * inside a (go ...) block. ports is a vector of channel endpoints,
 * which can be either a channel to take from or a vector of
 *   [channel-to-put-to val-to-put], in any combination. Takes will be
 *   made as if by <!, and puts will be made as if by >!. Unless
 *   the :priority option is true, if more than one port operation is
 *   ready a non-deterministic choice will be made. If no operation is
 *   ready and a :default value is supplied, [default-val :default] will
 *   be returned, otherwise alts! will park until the first operation to
 *   become ready completes. Returns [val port] of the completed
 *   operation, where val is the value taken for takes, and a
 *   boolean (true unless already closed, as per put!) for puts.
 * 
 *   opts are passed as :key val ... Supported options:
 * 
 *   :default val - the value to use if none of the operations are immediately ready
 *   :priority true - (default nil) when true, the operations will be tried in order.
 * 
 *   Note: there is no guarantee that the port exps or val exprs will be
 *   used, nor in what order should they be, so they should not be
 *   depended upon for side effects.
 */
cljs.core.async.alts_BANG_ = (function cljs$core$async$alts_BANG_(var_args){
var args__4742__auto__ = [];
var len__4736__auto___42710 = arguments.length;
var i__4737__auto___42711 = (0);
while(true){
if((i__4737__auto___42711 < len__4736__auto___42710)){
args__4742__auto__.push((arguments[i__4737__auto___42711]));

var G__42712 = (i__4737__auto___42711 + (1));
i__4737__auto___42711 = G__42712;
continue;
} else {
}
break;
}

var argseq__4743__auto__ = ((((1) < args__4742__auto__.length))?(new cljs.core.IndexedSeq(args__4742__auto__.slice((1)),(0),null)):null);
return cljs.core.async.alts_BANG_.cljs$core$IFn$_invoke$arity$variadic((arguments[(0)]),argseq__4743__auto__);
});

(cljs.core.async.alts_BANG_.cljs$core$IFn$_invoke$arity$variadic = (function (ports,p__42707){
var map__42708 = p__42707;
var map__42708__$1 = (((((!((map__42708 == null))))?(((((map__42708.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__42708.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__42708):map__42708);
var opts = map__42708__$1;
throw (new Error("alts! used not in (go ...) block"));
}));

(cljs.core.async.alts_BANG_.cljs$lang$maxFixedArity = (1));

/** @this {Function} */
(cljs.core.async.alts_BANG_.cljs$lang$applyTo = (function (seq42705){
var G__42706 = cljs.core.first.call(null,seq42705);
var seq42705__$1 = cljs.core.next.call(null,seq42705);
var self__4723__auto__ = this;
return self__4723__auto__.cljs$core$IFn$_invoke$arity$variadic(G__42706,seq42705__$1);
}));

/**
 * Puts a val into port if it's possible to do so immediately.
 *   nil values are not allowed. Never blocks. Returns true if offer succeeds.
 */
cljs.core.async.offer_BANG_ = (function cljs$core$async$offer_BANG_(port,val){
var ret = cljs.core.async.impl.protocols.put_BANG_.call(null,port,val,cljs.core.async.fn_handler.call(null,cljs.core.async.nop,false));
if(cljs.core.truth_(ret)){
return cljs.core.deref.call(null,ret);
} else {
return null;
}
});
/**
 * Takes a val from port if it's possible to do so immediately.
 *   Never blocks. Returns value if successful, nil otherwise.
 */
cljs.core.async.poll_BANG_ = (function cljs$core$async$poll_BANG_(port){
var ret = cljs.core.async.impl.protocols.take_BANG_.call(null,port,cljs.core.async.fn_handler.call(null,cljs.core.async.nop,false));
if(cljs.core.truth_(ret)){
return cljs.core.deref.call(null,ret);
} else {
return null;
}
});
/**
 * Takes elements from the from channel and supplies them to the to
 * channel. By default, the to channel will be closed when the from
 * channel closes, but can be determined by the close?  parameter. Will
 * stop consuming the from channel if the to channel closes
 */
cljs.core.async.pipe = (function cljs$core$async$pipe(var_args){
var G__42714 = arguments.length;
switch (G__42714) {
case 2:
return cljs.core.async.pipe.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.pipe.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.pipe.cljs$core$IFn$_invoke$arity$2 = (function (from,to){
return cljs.core.async.pipe.call(null,from,to,true);
}));

(cljs.core.async.pipe.cljs$core$IFn$_invoke$arity$3 = (function (from,to,close_QMARK_){
var c__26800__auto___42760 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_42738){
var state_val_42739 = (state_42738[(1)]);
if((state_val_42739 === (7))){
var inst_42734 = (state_42738[(2)]);
var state_42738__$1 = state_42738;
var statearr_42740_42761 = state_42738__$1;
(statearr_42740_42761[(2)] = inst_42734);

(statearr_42740_42761[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42739 === (1))){
var state_42738__$1 = state_42738;
var statearr_42741_42762 = state_42738__$1;
(statearr_42741_42762[(2)] = null);

(statearr_42741_42762[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42739 === (4))){
var inst_42717 = (state_42738[(7)]);
var inst_42717__$1 = (state_42738[(2)]);
var inst_42718 = (inst_42717__$1 == null);
var state_42738__$1 = (function (){var statearr_42742 = state_42738;
(statearr_42742[(7)] = inst_42717__$1);

return statearr_42742;
})();
if(cljs.core.truth_(inst_42718)){
var statearr_42743_42763 = state_42738__$1;
(statearr_42743_42763[(1)] = (5));

} else {
var statearr_42744_42764 = state_42738__$1;
(statearr_42744_42764[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42739 === (13))){
var state_42738__$1 = state_42738;
var statearr_42745_42765 = state_42738__$1;
(statearr_42745_42765[(2)] = null);

(statearr_42745_42765[(1)] = (14));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42739 === (6))){
var inst_42717 = (state_42738[(7)]);
var state_42738__$1 = state_42738;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_42738__$1,(11),to,inst_42717);
} else {
if((state_val_42739 === (3))){
var inst_42736 = (state_42738[(2)]);
var state_42738__$1 = state_42738;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_42738__$1,inst_42736);
} else {
if((state_val_42739 === (12))){
var state_42738__$1 = state_42738;
var statearr_42746_42766 = state_42738__$1;
(statearr_42746_42766[(2)] = null);

(statearr_42746_42766[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42739 === (2))){
var state_42738__$1 = state_42738;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_42738__$1,(4),from);
} else {
if((state_val_42739 === (11))){
var inst_42727 = (state_42738[(2)]);
var state_42738__$1 = state_42738;
if(cljs.core.truth_(inst_42727)){
var statearr_42747_42767 = state_42738__$1;
(statearr_42747_42767[(1)] = (12));

} else {
var statearr_42748_42768 = state_42738__$1;
(statearr_42748_42768[(1)] = (13));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42739 === (9))){
var state_42738__$1 = state_42738;
var statearr_42749_42769 = state_42738__$1;
(statearr_42749_42769[(2)] = null);

(statearr_42749_42769[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42739 === (5))){
var state_42738__$1 = state_42738;
if(cljs.core.truth_(close_QMARK_)){
var statearr_42750_42770 = state_42738__$1;
(statearr_42750_42770[(1)] = (8));

} else {
var statearr_42751_42771 = state_42738__$1;
(statearr_42751_42771[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42739 === (14))){
var inst_42732 = (state_42738[(2)]);
var state_42738__$1 = state_42738;
var statearr_42752_42772 = state_42738__$1;
(statearr_42752_42772[(2)] = inst_42732);

(statearr_42752_42772[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42739 === (10))){
var inst_42724 = (state_42738[(2)]);
var state_42738__$1 = state_42738;
var statearr_42753_42773 = state_42738__$1;
(statearr_42753_42773[(2)] = inst_42724);

(statearr_42753_42773[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42739 === (8))){
var inst_42721 = cljs.core.async.close_BANG_.call(null,to);
var state_42738__$1 = state_42738;
var statearr_42754_42774 = state_42738__$1;
(statearr_42754_42774[(2)] = inst_42721);

(statearr_42754_42774[(1)] = (10));


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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_42755 = [null,null,null,null,null,null,null,null];
(statearr_42755[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_42755[(1)] = (1));

return statearr_42755;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_42738){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_42738);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e42756){if((e42756 instanceof Object)){
var ex__26709__auto__ = e42756;
var statearr_42757_42775 = state_42738;
(statearr_42757_42775[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_42738);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e42756;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__42776 = state_42738;
state_42738 = G__42776;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_42738){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_42738);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_42758 = f__26801__auto__.call(null);
(statearr_42758[(6)] = c__26800__auto___42760);

return statearr_42758;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return to;
}));

(cljs.core.async.pipe.cljs$lang$maxFixedArity = 3);

cljs.core.async.pipeline_STAR_ = (function cljs$core$async$pipeline_STAR_(n,to,xf,from,close_QMARK_,ex_handler,type){
if((n > (0))){
} else {
throw (new Error("Assert failed: (pos? n)"));
}

var jobs = cljs.core.async.chan.call(null,n);
var results = cljs.core.async.chan.call(null,n);
var process__$1 = (function (p__42777){
var vec__42778 = p__42777;
var v = cljs.core.nth.call(null,vec__42778,(0),null);
var p = cljs.core.nth.call(null,vec__42778,(1),null);
var job = vec__42778;
if((job == null)){
cljs.core.async.close_BANG_.call(null,results);

return null;
} else {
var res = cljs.core.async.chan.call(null,(1),xf,ex_handler);
var c__26800__auto___42949 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_42785){
var state_val_42786 = (state_42785[(1)]);
if((state_val_42786 === (1))){
var state_42785__$1 = state_42785;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_42785__$1,(2),res,v);
} else {
if((state_val_42786 === (2))){
var inst_42782 = (state_42785[(2)]);
var inst_42783 = cljs.core.async.close_BANG_.call(null,res);
var state_42785__$1 = (function (){var statearr_42787 = state_42785;
(statearr_42787[(7)] = inst_42782);

return statearr_42787;
})();
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_42785__$1,inst_42783);
} else {
return null;
}
}
});
return (function() {
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__ = null;
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0 = (function (){
var statearr_42788 = [null,null,null,null,null,null,null,null];
(statearr_42788[(0)] = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__);

(statearr_42788[(1)] = (1));

return statearr_42788;
});
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1 = (function (state_42785){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_42785);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e42789){if((e42789 instanceof Object)){
var ex__26709__auto__ = e42789;
var statearr_42790_42950 = state_42785;
(statearr_42790_42950[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_42785);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e42789;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__42951 = state_42785;
state_42785 = G__42951;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__ = function(state_42785){
switch(arguments.length){
case 0:
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1.call(this,state_42785);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0;
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1;
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_42791 = f__26801__auto__.call(null);
(statearr_42791[(6)] = c__26800__auto___42949);

return statearr_42791;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


cljs.core.async.put_BANG_.call(null,p,res);

return true;
}
});
var async = (function (p__42792){
var vec__42793 = p__42792;
var v = cljs.core.nth.call(null,vec__42793,(0),null);
var p = cljs.core.nth.call(null,vec__42793,(1),null);
var job = vec__42793;
if((job == null)){
cljs.core.async.close_BANG_.call(null,results);

return null;
} else {
var res = cljs.core.async.chan.call(null,(1));
xf.call(null,v,res);

cljs.core.async.put_BANG_.call(null,p,res);

return true;
}
});
var n__4613__auto___42952 = n;
var __42953 = (0);
while(true){
if((__42953 < n__4613__auto___42952)){
var G__42796_42954 = type;
var G__42796_42955__$1 = (((G__42796_42954 instanceof cljs.core.Keyword))?G__42796_42954.fqn:null);
switch (G__42796_42955__$1) {
case "compute":
var c__26800__auto___42957 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,((function (__42953,c__26800__auto___42957,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async){
return (function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = ((function (__42953,c__26800__auto___42957,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async){
return (function (state_42809){
var state_val_42810 = (state_42809[(1)]);
if((state_val_42810 === (1))){
var state_42809__$1 = state_42809;
var statearr_42811_42958 = state_42809__$1;
(statearr_42811_42958[(2)] = null);

(statearr_42811_42958[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42810 === (2))){
var state_42809__$1 = state_42809;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_42809__$1,(4),jobs);
} else {
if((state_val_42810 === (3))){
var inst_42807 = (state_42809[(2)]);
var state_42809__$1 = state_42809;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_42809__$1,inst_42807);
} else {
if((state_val_42810 === (4))){
var inst_42799 = (state_42809[(2)]);
var inst_42800 = process__$1.call(null,inst_42799);
var state_42809__$1 = state_42809;
if(cljs.core.truth_(inst_42800)){
var statearr_42812_42959 = state_42809__$1;
(statearr_42812_42959[(1)] = (5));

} else {
var statearr_42813_42960 = state_42809__$1;
(statearr_42813_42960[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42810 === (5))){
var state_42809__$1 = state_42809;
var statearr_42814_42961 = state_42809__$1;
(statearr_42814_42961[(2)] = null);

(statearr_42814_42961[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42810 === (6))){
var state_42809__$1 = state_42809;
var statearr_42815_42962 = state_42809__$1;
(statearr_42815_42962[(2)] = null);

(statearr_42815_42962[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42810 === (7))){
var inst_42805 = (state_42809[(2)]);
var state_42809__$1 = state_42809;
var statearr_42816_42963 = state_42809__$1;
(statearr_42816_42963[(2)] = inst_42805);

(statearr_42816_42963[(1)] = (3));


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
});})(__42953,c__26800__auto___42957,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async))
;
return ((function (__42953,switch__26705__auto__,c__26800__auto___42957,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async){
return (function() {
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__ = null;
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0 = (function (){
var statearr_42817 = [null,null,null,null,null,null,null];
(statearr_42817[(0)] = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__);

(statearr_42817[(1)] = (1));

return statearr_42817;
});
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1 = (function (state_42809){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_42809);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e42818){if((e42818 instanceof Object)){
var ex__26709__auto__ = e42818;
var statearr_42819_42964 = state_42809;
(statearr_42819_42964[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_42809);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e42818;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__42965 = state_42809;
state_42809 = G__42965;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__ = function(state_42809){
switch(arguments.length){
case 0:
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1.call(this,state_42809);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0;
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1;
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__;
})()
;})(__42953,switch__26705__auto__,c__26800__auto___42957,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async))
})();
var state__26802__auto__ = (function (){var statearr_42820 = f__26801__auto__.call(null);
(statearr_42820[(6)] = c__26800__auto___42957);

return statearr_42820;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
});})(__42953,c__26800__auto___42957,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async))
);


break;
case "async":
var c__26800__auto___42966 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,((function (__42953,c__26800__auto___42966,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async){
return (function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = ((function (__42953,c__26800__auto___42966,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async){
return (function (state_42833){
var state_val_42834 = (state_42833[(1)]);
if((state_val_42834 === (1))){
var state_42833__$1 = state_42833;
var statearr_42835_42967 = state_42833__$1;
(statearr_42835_42967[(2)] = null);

(statearr_42835_42967[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42834 === (2))){
var state_42833__$1 = state_42833;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_42833__$1,(4),jobs);
} else {
if((state_val_42834 === (3))){
var inst_42831 = (state_42833[(2)]);
var state_42833__$1 = state_42833;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_42833__$1,inst_42831);
} else {
if((state_val_42834 === (4))){
var inst_42823 = (state_42833[(2)]);
var inst_42824 = async.call(null,inst_42823);
var state_42833__$1 = state_42833;
if(cljs.core.truth_(inst_42824)){
var statearr_42836_42968 = state_42833__$1;
(statearr_42836_42968[(1)] = (5));

} else {
var statearr_42837_42969 = state_42833__$1;
(statearr_42837_42969[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42834 === (5))){
var state_42833__$1 = state_42833;
var statearr_42838_42970 = state_42833__$1;
(statearr_42838_42970[(2)] = null);

(statearr_42838_42970[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42834 === (6))){
var state_42833__$1 = state_42833;
var statearr_42839_42971 = state_42833__$1;
(statearr_42839_42971[(2)] = null);

(statearr_42839_42971[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42834 === (7))){
var inst_42829 = (state_42833[(2)]);
var state_42833__$1 = state_42833;
var statearr_42840_42972 = state_42833__$1;
(statearr_42840_42972[(2)] = inst_42829);

(statearr_42840_42972[(1)] = (3));


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
});})(__42953,c__26800__auto___42966,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async))
;
return ((function (__42953,switch__26705__auto__,c__26800__auto___42966,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async){
return (function() {
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__ = null;
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0 = (function (){
var statearr_42841 = [null,null,null,null,null,null,null];
(statearr_42841[(0)] = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__);

(statearr_42841[(1)] = (1));

return statearr_42841;
});
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1 = (function (state_42833){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_42833);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e42842){if((e42842 instanceof Object)){
var ex__26709__auto__ = e42842;
var statearr_42843_42973 = state_42833;
(statearr_42843_42973[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_42833);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e42842;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__42974 = state_42833;
state_42833 = G__42974;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__ = function(state_42833){
switch(arguments.length){
case 0:
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1.call(this,state_42833);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0;
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1;
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__;
})()
;})(__42953,switch__26705__auto__,c__26800__auto___42966,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async))
})();
var state__26802__auto__ = (function (){var statearr_42844 = f__26801__auto__.call(null);
(statearr_42844[(6)] = c__26800__auto___42966);

return statearr_42844;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
});})(__42953,c__26800__auto___42966,G__42796_42954,G__42796_42955__$1,n__4613__auto___42952,jobs,results,process__$1,async))
);


break;
default:
throw (new Error(["No matching clause: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(G__42796_42955__$1)].join('')));

}

var G__42975 = (__42953 + (1));
__42953 = G__42975;
continue;
} else {
}
break;
}

var c__26800__auto___42976 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_42866){
var state_val_42867 = (state_42866[(1)]);
if((state_val_42867 === (7))){
var inst_42862 = (state_42866[(2)]);
var state_42866__$1 = state_42866;
var statearr_42868_42977 = state_42866__$1;
(statearr_42868_42977[(2)] = inst_42862);

(statearr_42868_42977[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42867 === (1))){
var state_42866__$1 = state_42866;
var statearr_42869_42978 = state_42866__$1;
(statearr_42869_42978[(2)] = null);

(statearr_42869_42978[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42867 === (4))){
var inst_42847 = (state_42866[(7)]);
var inst_42847__$1 = (state_42866[(2)]);
var inst_42848 = (inst_42847__$1 == null);
var state_42866__$1 = (function (){var statearr_42870 = state_42866;
(statearr_42870[(7)] = inst_42847__$1);

return statearr_42870;
})();
if(cljs.core.truth_(inst_42848)){
var statearr_42871_42979 = state_42866__$1;
(statearr_42871_42979[(1)] = (5));

} else {
var statearr_42872_42980 = state_42866__$1;
(statearr_42872_42980[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42867 === (6))){
var inst_42847 = (state_42866[(7)]);
var inst_42852 = (state_42866[(8)]);
var inst_42852__$1 = cljs.core.async.chan.call(null,(1));
var inst_42853 = cljs.core.PersistentVector.EMPTY_NODE;
var inst_42854 = [inst_42847,inst_42852__$1];
var inst_42855 = (new cljs.core.PersistentVector(null,2,(5),inst_42853,inst_42854,null));
var state_42866__$1 = (function (){var statearr_42873 = state_42866;
(statearr_42873[(8)] = inst_42852__$1);

return statearr_42873;
})();
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_42866__$1,(8),jobs,inst_42855);
} else {
if((state_val_42867 === (3))){
var inst_42864 = (state_42866[(2)]);
var state_42866__$1 = state_42866;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_42866__$1,inst_42864);
} else {
if((state_val_42867 === (2))){
var state_42866__$1 = state_42866;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_42866__$1,(4),from);
} else {
if((state_val_42867 === (9))){
var inst_42859 = (state_42866[(2)]);
var state_42866__$1 = (function (){var statearr_42874 = state_42866;
(statearr_42874[(9)] = inst_42859);

return statearr_42874;
})();
var statearr_42875_42981 = state_42866__$1;
(statearr_42875_42981[(2)] = null);

(statearr_42875_42981[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42867 === (5))){
var inst_42850 = cljs.core.async.close_BANG_.call(null,jobs);
var state_42866__$1 = state_42866;
var statearr_42876_42982 = state_42866__$1;
(statearr_42876_42982[(2)] = inst_42850);

(statearr_42876_42982[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42867 === (8))){
var inst_42852 = (state_42866[(8)]);
var inst_42857 = (state_42866[(2)]);
var state_42866__$1 = (function (){var statearr_42877 = state_42866;
(statearr_42877[(10)] = inst_42857);

return statearr_42877;
})();
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_42866__$1,(9),results,inst_42852);
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
});
return (function() {
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__ = null;
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0 = (function (){
var statearr_42878 = [null,null,null,null,null,null,null,null,null,null,null];
(statearr_42878[(0)] = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__);

(statearr_42878[(1)] = (1));

return statearr_42878;
});
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1 = (function (state_42866){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_42866);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e42879){if((e42879 instanceof Object)){
var ex__26709__auto__ = e42879;
var statearr_42880_42983 = state_42866;
(statearr_42880_42983[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_42866);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e42879;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__42984 = state_42866;
state_42866 = G__42984;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__ = function(state_42866){
switch(arguments.length){
case 0:
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1.call(this,state_42866);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0;
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1;
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_42881 = f__26801__auto__.call(null);
(statearr_42881[(6)] = c__26800__auto___42976);

return statearr_42881;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


var c__26800__auto__ = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_42919){
var state_val_42920 = (state_42919[(1)]);
if((state_val_42920 === (7))){
var inst_42915 = (state_42919[(2)]);
var state_42919__$1 = state_42919;
var statearr_42921_42985 = state_42919__$1;
(statearr_42921_42985[(2)] = inst_42915);

(statearr_42921_42985[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (20))){
var state_42919__$1 = state_42919;
var statearr_42922_42986 = state_42919__$1;
(statearr_42922_42986[(2)] = null);

(statearr_42922_42986[(1)] = (21));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (1))){
var state_42919__$1 = state_42919;
var statearr_42923_42987 = state_42919__$1;
(statearr_42923_42987[(2)] = null);

(statearr_42923_42987[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (4))){
var inst_42884 = (state_42919[(7)]);
var inst_42884__$1 = (state_42919[(2)]);
var inst_42885 = (inst_42884__$1 == null);
var state_42919__$1 = (function (){var statearr_42924 = state_42919;
(statearr_42924[(7)] = inst_42884__$1);

return statearr_42924;
})();
if(cljs.core.truth_(inst_42885)){
var statearr_42925_42988 = state_42919__$1;
(statearr_42925_42988[(1)] = (5));

} else {
var statearr_42926_42989 = state_42919__$1;
(statearr_42926_42989[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (15))){
var inst_42897 = (state_42919[(8)]);
var state_42919__$1 = state_42919;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_42919__$1,(18),to,inst_42897);
} else {
if((state_val_42920 === (21))){
var inst_42910 = (state_42919[(2)]);
var state_42919__$1 = state_42919;
var statearr_42927_42990 = state_42919__$1;
(statearr_42927_42990[(2)] = inst_42910);

(statearr_42927_42990[(1)] = (13));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (13))){
var inst_42912 = (state_42919[(2)]);
var state_42919__$1 = (function (){var statearr_42928 = state_42919;
(statearr_42928[(9)] = inst_42912);

return statearr_42928;
})();
var statearr_42929_42991 = state_42919__$1;
(statearr_42929_42991[(2)] = null);

(statearr_42929_42991[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (6))){
var inst_42884 = (state_42919[(7)]);
var state_42919__$1 = state_42919;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_42919__$1,(11),inst_42884);
} else {
if((state_val_42920 === (17))){
var inst_42905 = (state_42919[(2)]);
var state_42919__$1 = state_42919;
if(cljs.core.truth_(inst_42905)){
var statearr_42930_42992 = state_42919__$1;
(statearr_42930_42992[(1)] = (19));

} else {
var statearr_42931_42993 = state_42919__$1;
(statearr_42931_42993[(1)] = (20));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (3))){
var inst_42917 = (state_42919[(2)]);
var state_42919__$1 = state_42919;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_42919__$1,inst_42917);
} else {
if((state_val_42920 === (12))){
var inst_42894 = (state_42919[(10)]);
var state_42919__$1 = state_42919;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_42919__$1,(14),inst_42894);
} else {
if((state_val_42920 === (2))){
var state_42919__$1 = state_42919;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_42919__$1,(4),results);
} else {
if((state_val_42920 === (19))){
var state_42919__$1 = state_42919;
var statearr_42932_42994 = state_42919__$1;
(statearr_42932_42994[(2)] = null);

(statearr_42932_42994[(1)] = (12));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (11))){
var inst_42894 = (state_42919[(2)]);
var state_42919__$1 = (function (){var statearr_42933 = state_42919;
(statearr_42933[(10)] = inst_42894);

return statearr_42933;
})();
var statearr_42934_42995 = state_42919__$1;
(statearr_42934_42995[(2)] = null);

(statearr_42934_42995[(1)] = (12));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (9))){
var state_42919__$1 = state_42919;
var statearr_42935_42996 = state_42919__$1;
(statearr_42935_42996[(2)] = null);

(statearr_42935_42996[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (5))){
var state_42919__$1 = state_42919;
if(cljs.core.truth_(close_QMARK_)){
var statearr_42936_42997 = state_42919__$1;
(statearr_42936_42997[(1)] = (8));

} else {
var statearr_42937_42998 = state_42919__$1;
(statearr_42937_42998[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (14))){
var inst_42897 = (state_42919[(8)]);
var inst_42897__$1 = (state_42919[(2)]);
var inst_42898 = (inst_42897__$1 == null);
var inst_42899 = cljs.core.not.call(null,inst_42898);
var state_42919__$1 = (function (){var statearr_42938 = state_42919;
(statearr_42938[(8)] = inst_42897__$1);

return statearr_42938;
})();
if(inst_42899){
var statearr_42939_42999 = state_42919__$1;
(statearr_42939_42999[(1)] = (15));

} else {
var statearr_42940_43000 = state_42919__$1;
(statearr_42940_43000[(1)] = (16));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (16))){
var state_42919__$1 = state_42919;
var statearr_42941_43001 = state_42919__$1;
(statearr_42941_43001[(2)] = false);

(statearr_42941_43001[(1)] = (17));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (10))){
var inst_42891 = (state_42919[(2)]);
var state_42919__$1 = state_42919;
var statearr_42942_43002 = state_42919__$1;
(statearr_42942_43002[(2)] = inst_42891);

(statearr_42942_43002[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (18))){
var inst_42902 = (state_42919[(2)]);
var state_42919__$1 = state_42919;
var statearr_42943_43003 = state_42919__$1;
(statearr_42943_43003[(2)] = inst_42902);

(statearr_42943_43003[(1)] = (17));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_42920 === (8))){
var inst_42888 = cljs.core.async.close_BANG_.call(null,to);
var state_42919__$1 = state_42919;
var statearr_42944_43004 = state_42919__$1;
(statearr_42944_43004[(2)] = inst_42888);

(statearr_42944_43004[(1)] = (10));


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
});
return (function() {
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__ = null;
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0 = (function (){
var statearr_42945 = [null,null,null,null,null,null,null,null,null,null,null];
(statearr_42945[(0)] = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__);

(statearr_42945[(1)] = (1));

return statearr_42945;
});
var cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1 = (function (state_42919){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_42919);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e42946){if((e42946 instanceof Object)){
var ex__26709__auto__ = e42946;
var statearr_42947_43005 = state_42919;
(statearr_42947_43005[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_42919);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e42946;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__43006 = state_42919;
state_42919 = G__43006;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__ = function(state_42919){
switch(arguments.length){
case 0:
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1.call(this,state_42919);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____0;
cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$pipeline_STAR__$_state_machine__26706__auto____1;
return cljs$core$async$pipeline_STAR__$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_42948 = f__26801__auto__.call(null);
(statearr_42948[(6)] = c__26800__auto__);

return statearr_42948;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));

return c__26800__auto__;
});
/**
 * Takes elements from the from channel and supplies them to the to
 *   channel, subject to the async function af, with parallelism n. af
 *   must be a function of two arguments, the first an input value and
 *   the second a channel on which to place the result(s). af must close!
 *   the channel before returning.  The presumption is that af will
 *   return immediately, having launched some asynchronous operation
 *   whose completion/callback will manipulate the result channel. Outputs
 *   will be returned in order relative to  the inputs. By default, the to
 *   channel will be closed when the from channel closes, but can be
 *   determined by the close?  parameter. Will stop consuming the from
 *   channel if the to channel closes.
 */
cljs.core.async.pipeline_async = (function cljs$core$async$pipeline_async(var_args){
var G__43008 = arguments.length;
switch (G__43008) {
case 4:
return cljs.core.async.pipeline_async.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
case 5:
return cljs.core.async.pipeline_async.cljs$core$IFn$_invoke$arity$5((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]),(arguments[(4)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.pipeline_async.cljs$core$IFn$_invoke$arity$4 = (function (n,to,af,from){
return cljs.core.async.pipeline_async.call(null,n,to,af,from,true);
}));

(cljs.core.async.pipeline_async.cljs$core$IFn$_invoke$arity$5 = (function (n,to,af,from,close_QMARK_){
return cljs.core.async.pipeline_STAR_.call(null,n,to,af,from,close_QMARK_,null,new cljs.core.Keyword(null,"async","async",1050769601));
}));

(cljs.core.async.pipeline_async.cljs$lang$maxFixedArity = 5);

/**
 * Takes elements from the from channel and supplies them to the to
 *   channel, subject to the transducer xf, with parallelism n. Because
 *   it is parallel, the transducer will be applied independently to each
 *   element, not across elements, and may produce zero or more outputs
 *   per input.  Outputs will be returned in order relative to the
 *   inputs. By default, the to channel will be closed when the from
 *   channel closes, but can be determined by the close?  parameter. Will
 *   stop consuming the from channel if the to channel closes.
 * 
 *   Note this is supplied for API compatibility with the Clojure version.
 *   Values of N > 1 will not result in actual concurrency in a
 *   single-threaded runtime.
 */
cljs.core.async.pipeline = (function cljs$core$async$pipeline(var_args){
var G__43011 = arguments.length;
switch (G__43011) {
case 4:
return cljs.core.async.pipeline.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
case 5:
return cljs.core.async.pipeline.cljs$core$IFn$_invoke$arity$5((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]),(arguments[(4)]));

break;
case 6:
return cljs.core.async.pipeline.cljs$core$IFn$_invoke$arity$6((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]),(arguments[(4)]),(arguments[(5)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.pipeline.cljs$core$IFn$_invoke$arity$4 = (function (n,to,xf,from){
return cljs.core.async.pipeline.call(null,n,to,xf,from,true);
}));

(cljs.core.async.pipeline.cljs$core$IFn$_invoke$arity$5 = (function (n,to,xf,from,close_QMARK_){
return cljs.core.async.pipeline.call(null,n,to,xf,from,close_QMARK_,null);
}));

(cljs.core.async.pipeline.cljs$core$IFn$_invoke$arity$6 = (function (n,to,xf,from,close_QMARK_,ex_handler){
return cljs.core.async.pipeline_STAR_.call(null,n,to,xf,from,close_QMARK_,ex_handler,new cljs.core.Keyword(null,"compute","compute",1555393130));
}));

(cljs.core.async.pipeline.cljs$lang$maxFixedArity = 6);

/**
 * Takes a predicate and a source channel and returns a vector of two
 *   channels, the first of which will contain the values for which the
 *   predicate returned true, the second those for which it returned
 *   false.
 * 
 *   The out channels will be unbuffered by default, or two buf-or-ns can
 *   be supplied. The channels will close after the source channel has
 *   closed.
 */
cljs.core.async.split = (function cljs$core$async$split(var_args){
var G__43014 = arguments.length;
switch (G__43014) {
case 2:
return cljs.core.async.split.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 4:
return cljs.core.async.split.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.split.cljs$core$IFn$_invoke$arity$2 = (function (p,ch){
return cljs.core.async.split.call(null,p,ch,null,null);
}));

(cljs.core.async.split.cljs$core$IFn$_invoke$arity$4 = (function (p,ch,t_buf_or_n,f_buf_or_n){
var tc = cljs.core.async.chan.call(null,t_buf_or_n);
var fc = cljs.core.async.chan.call(null,f_buf_or_n);
var c__26800__auto___43063 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_43040){
var state_val_43041 = (state_43040[(1)]);
if((state_val_43041 === (7))){
var inst_43036 = (state_43040[(2)]);
var state_43040__$1 = state_43040;
var statearr_43042_43064 = state_43040__$1;
(statearr_43042_43064[(2)] = inst_43036);

(statearr_43042_43064[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43041 === (1))){
var state_43040__$1 = state_43040;
var statearr_43043_43065 = state_43040__$1;
(statearr_43043_43065[(2)] = null);

(statearr_43043_43065[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43041 === (4))){
var inst_43017 = (state_43040[(7)]);
var inst_43017__$1 = (state_43040[(2)]);
var inst_43018 = (inst_43017__$1 == null);
var state_43040__$1 = (function (){var statearr_43044 = state_43040;
(statearr_43044[(7)] = inst_43017__$1);

return statearr_43044;
})();
if(cljs.core.truth_(inst_43018)){
var statearr_43045_43066 = state_43040__$1;
(statearr_43045_43066[(1)] = (5));

} else {
var statearr_43046_43067 = state_43040__$1;
(statearr_43046_43067[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43041 === (13))){
var state_43040__$1 = state_43040;
var statearr_43047_43068 = state_43040__$1;
(statearr_43047_43068[(2)] = null);

(statearr_43047_43068[(1)] = (14));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43041 === (6))){
var inst_43017 = (state_43040[(7)]);
var inst_43023 = p.call(null,inst_43017);
var state_43040__$1 = state_43040;
if(cljs.core.truth_(inst_43023)){
var statearr_43048_43069 = state_43040__$1;
(statearr_43048_43069[(1)] = (9));

} else {
var statearr_43049_43070 = state_43040__$1;
(statearr_43049_43070[(1)] = (10));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43041 === (3))){
var inst_43038 = (state_43040[(2)]);
var state_43040__$1 = state_43040;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_43040__$1,inst_43038);
} else {
if((state_val_43041 === (12))){
var state_43040__$1 = state_43040;
var statearr_43050_43071 = state_43040__$1;
(statearr_43050_43071[(2)] = null);

(statearr_43050_43071[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43041 === (2))){
var state_43040__$1 = state_43040;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_43040__$1,(4),ch);
} else {
if((state_val_43041 === (11))){
var inst_43017 = (state_43040[(7)]);
var inst_43027 = (state_43040[(2)]);
var state_43040__$1 = state_43040;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_43040__$1,(8),inst_43027,inst_43017);
} else {
if((state_val_43041 === (9))){
var state_43040__$1 = state_43040;
var statearr_43051_43072 = state_43040__$1;
(statearr_43051_43072[(2)] = tc);

(statearr_43051_43072[(1)] = (11));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43041 === (5))){
var inst_43020 = cljs.core.async.close_BANG_.call(null,tc);
var inst_43021 = cljs.core.async.close_BANG_.call(null,fc);
var state_43040__$1 = (function (){var statearr_43052 = state_43040;
(statearr_43052[(8)] = inst_43020);

return statearr_43052;
})();
var statearr_43053_43073 = state_43040__$1;
(statearr_43053_43073[(2)] = inst_43021);

(statearr_43053_43073[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43041 === (14))){
var inst_43034 = (state_43040[(2)]);
var state_43040__$1 = state_43040;
var statearr_43054_43074 = state_43040__$1;
(statearr_43054_43074[(2)] = inst_43034);

(statearr_43054_43074[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43041 === (10))){
var state_43040__$1 = state_43040;
var statearr_43055_43075 = state_43040__$1;
(statearr_43055_43075[(2)] = fc);

(statearr_43055_43075[(1)] = (11));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43041 === (8))){
var inst_43029 = (state_43040[(2)]);
var state_43040__$1 = state_43040;
if(cljs.core.truth_(inst_43029)){
var statearr_43056_43076 = state_43040__$1;
(statearr_43056_43076[(1)] = (12));

} else {
var statearr_43057_43077 = state_43040__$1;
(statearr_43057_43077[(1)] = (13));

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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_43058 = [null,null,null,null,null,null,null,null,null];
(statearr_43058[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_43058[(1)] = (1));

return statearr_43058;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_43040){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_43040);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e43059){if((e43059 instanceof Object)){
var ex__26709__auto__ = e43059;
var statearr_43060_43078 = state_43040;
(statearr_43060_43078[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_43040);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e43059;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__43079 = state_43040;
state_43040 = G__43079;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_43040){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_43040);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_43061 = f__26801__auto__.call(null);
(statearr_43061[(6)] = c__26800__auto___43063);

return statearr_43061;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return new cljs.core.PersistentVector(null, 2, 5, cljs.core.PersistentVector.EMPTY_NODE, [tc,fc], null);
}));

(cljs.core.async.split.cljs$lang$maxFixedArity = 4);

/**
 * f should be a function of 2 arguments. Returns a channel containing
 *   the single result of applying f to init and the first item from the
 *   channel, then applying f to that result and the 2nd item, etc. If
 *   the channel closes without yielding items, returns init and f is not
 *   called. ch must close before reduce produces a result.
 */
cljs.core.async.reduce = (function cljs$core$async$reduce(f,init,ch){
var c__26800__auto__ = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_43100){
var state_val_43101 = (state_43100[(1)]);
if((state_val_43101 === (7))){
var inst_43096 = (state_43100[(2)]);
var state_43100__$1 = state_43100;
var statearr_43102_43120 = state_43100__$1;
(statearr_43102_43120[(2)] = inst_43096);

(statearr_43102_43120[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43101 === (1))){
var inst_43080 = init;
var state_43100__$1 = (function (){var statearr_43103 = state_43100;
(statearr_43103[(7)] = inst_43080);

return statearr_43103;
})();
var statearr_43104_43121 = state_43100__$1;
(statearr_43104_43121[(2)] = null);

(statearr_43104_43121[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43101 === (4))){
var inst_43083 = (state_43100[(8)]);
var inst_43083__$1 = (state_43100[(2)]);
var inst_43084 = (inst_43083__$1 == null);
var state_43100__$1 = (function (){var statearr_43105 = state_43100;
(statearr_43105[(8)] = inst_43083__$1);

return statearr_43105;
})();
if(cljs.core.truth_(inst_43084)){
var statearr_43106_43122 = state_43100__$1;
(statearr_43106_43122[(1)] = (5));

} else {
var statearr_43107_43123 = state_43100__$1;
(statearr_43107_43123[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43101 === (6))){
var inst_43080 = (state_43100[(7)]);
var inst_43083 = (state_43100[(8)]);
var inst_43087 = (state_43100[(9)]);
var inst_43087__$1 = f.call(null,inst_43080,inst_43083);
var inst_43088 = cljs.core.reduced_QMARK_.call(null,inst_43087__$1);
var state_43100__$1 = (function (){var statearr_43108 = state_43100;
(statearr_43108[(9)] = inst_43087__$1);

return statearr_43108;
})();
if(inst_43088){
var statearr_43109_43124 = state_43100__$1;
(statearr_43109_43124[(1)] = (8));

} else {
var statearr_43110_43125 = state_43100__$1;
(statearr_43110_43125[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43101 === (3))){
var inst_43098 = (state_43100[(2)]);
var state_43100__$1 = state_43100;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_43100__$1,inst_43098);
} else {
if((state_val_43101 === (2))){
var state_43100__$1 = state_43100;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_43100__$1,(4),ch);
} else {
if((state_val_43101 === (9))){
var inst_43087 = (state_43100[(9)]);
var inst_43080 = inst_43087;
var state_43100__$1 = (function (){var statearr_43111 = state_43100;
(statearr_43111[(7)] = inst_43080);

return statearr_43111;
})();
var statearr_43112_43126 = state_43100__$1;
(statearr_43112_43126[(2)] = null);

(statearr_43112_43126[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43101 === (5))){
var inst_43080 = (state_43100[(7)]);
var state_43100__$1 = state_43100;
var statearr_43113_43127 = state_43100__$1;
(statearr_43113_43127[(2)] = inst_43080);

(statearr_43113_43127[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43101 === (10))){
var inst_43094 = (state_43100[(2)]);
var state_43100__$1 = state_43100;
var statearr_43114_43128 = state_43100__$1;
(statearr_43114_43128[(2)] = inst_43094);

(statearr_43114_43128[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43101 === (8))){
var inst_43087 = (state_43100[(9)]);
var inst_43090 = cljs.core.deref.call(null,inst_43087);
var state_43100__$1 = state_43100;
var statearr_43115_43129 = state_43100__$1;
(statearr_43115_43129[(2)] = inst_43090);

(statearr_43115_43129[(1)] = (10));


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
});
return (function() {
var cljs$core$async$reduce_$_state_machine__26706__auto__ = null;
var cljs$core$async$reduce_$_state_machine__26706__auto____0 = (function (){
var statearr_43116 = [null,null,null,null,null,null,null,null,null,null];
(statearr_43116[(0)] = cljs$core$async$reduce_$_state_machine__26706__auto__);

(statearr_43116[(1)] = (1));

return statearr_43116;
});
var cljs$core$async$reduce_$_state_machine__26706__auto____1 = (function (state_43100){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_43100);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e43117){if((e43117 instanceof Object)){
var ex__26709__auto__ = e43117;
var statearr_43118_43130 = state_43100;
(statearr_43118_43130[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_43100);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e43117;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__43131 = state_43100;
state_43100 = G__43131;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$reduce_$_state_machine__26706__auto__ = function(state_43100){
switch(arguments.length){
case 0:
return cljs$core$async$reduce_$_state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$reduce_$_state_machine__26706__auto____1.call(this,state_43100);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$reduce_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$reduce_$_state_machine__26706__auto____0;
cljs$core$async$reduce_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$reduce_$_state_machine__26706__auto____1;
return cljs$core$async$reduce_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_43119 = f__26801__auto__.call(null);
(statearr_43119[(6)] = c__26800__auto__);

return statearr_43119;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));

return c__26800__auto__;
});
/**
 * async/reduces a channel with a transformation (xform f).
 *   Returns a channel containing the result.  ch must close before
 *   transduce produces a result.
 */
cljs.core.async.transduce = (function cljs$core$async$transduce(xform,f,init,ch){
var f__$1 = xform.call(null,f);
var c__26800__auto__ = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_43137){
var state_val_43138 = (state_43137[(1)]);
if((state_val_43138 === (1))){
var inst_43132 = cljs.core.async.reduce.call(null,f__$1,init,ch);
var state_43137__$1 = state_43137;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_43137__$1,(2),inst_43132);
} else {
if((state_val_43138 === (2))){
var inst_43134 = (state_43137[(2)]);
var inst_43135 = f__$1.call(null,inst_43134);
var state_43137__$1 = state_43137;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_43137__$1,inst_43135);
} else {
return null;
}
}
});
return (function() {
var cljs$core$async$transduce_$_state_machine__26706__auto__ = null;
var cljs$core$async$transduce_$_state_machine__26706__auto____0 = (function (){
var statearr_43139 = [null,null,null,null,null,null,null];
(statearr_43139[(0)] = cljs$core$async$transduce_$_state_machine__26706__auto__);

(statearr_43139[(1)] = (1));

return statearr_43139;
});
var cljs$core$async$transduce_$_state_machine__26706__auto____1 = (function (state_43137){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_43137);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e43140){if((e43140 instanceof Object)){
var ex__26709__auto__ = e43140;
var statearr_43141_43143 = state_43137;
(statearr_43141_43143[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_43137);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e43140;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__43144 = state_43137;
state_43137 = G__43144;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$transduce_$_state_machine__26706__auto__ = function(state_43137){
switch(arguments.length){
case 0:
return cljs$core$async$transduce_$_state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$transduce_$_state_machine__26706__auto____1.call(this,state_43137);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$transduce_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$transduce_$_state_machine__26706__auto____0;
cljs$core$async$transduce_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$transduce_$_state_machine__26706__auto____1;
return cljs$core$async$transduce_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_43142 = f__26801__auto__.call(null);
(statearr_43142[(6)] = c__26800__auto__);

return statearr_43142;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));

return c__26800__auto__;
});
/**
 * Puts the contents of coll into the supplied channel.
 * 
 *   By default the channel will be closed after the items are copied,
 *   but can be determined by the close? parameter.
 * 
 *   Returns a channel which will close after the items are copied.
 */
cljs.core.async.onto_chan = (function cljs$core$async$onto_chan(var_args){
var G__43146 = arguments.length;
switch (G__43146) {
case 2:
return cljs.core.async.onto_chan.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.onto_chan.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.onto_chan.cljs$core$IFn$_invoke$arity$2 = (function (ch,coll){
return cljs.core.async.onto_chan.call(null,ch,coll,true);
}));

(cljs.core.async.onto_chan.cljs$core$IFn$_invoke$arity$3 = (function (ch,coll,close_QMARK_){
var c__26800__auto__ = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_43171){
var state_val_43172 = (state_43171[(1)]);
if((state_val_43172 === (7))){
var inst_43153 = (state_43171[(2)]);
var state_43171__$1 = state_43171;
var statearr_43173_43194 = state_43171__$1;
(statearr_43173_43194[(2)] = inst_43153);

(statearr_43173_43194[(1)] = (6));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43172 === (1))){
var inst_43147 = cljs.core.seq.call(null,coll);
var inst_43148 = inst_43147;
var state_43171__$1 = (function (){var statearr_43174 = state_43171;
(statearr_43174[(7)] = inst_43148);

return statearr_43174;
})();
var statearr_43175_43195 = state_43171__$1;
(statearr_43175_43195[(2)] = null);

(statearr_43175_43195[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43172 === (4))){
var inst_43148 = (state_43171[(7)]);
var inst_43151 = cljs.core.first.call(null,inst_43148);
var state_43171__$1 = state_43171;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_43171__$1,(7),ch,inst_43151);
} else {
if((state_val_43172 === (13))){
var inst_43165 = (state_43171[(2)]);
var state_43171__$1 = state_43171;
var statearr_43176_43196 = state_43171__$1;
(statearr_43176_43196[(2)] = inst_43165);

(statearr_43176_43196[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43172 === (6))){
var inst_43156 = (state_43171[(2)]);
var state_43171__$1 = state_43171;
if(cljs.core.truth_(inst_43156)){
var statearr_43177_43197 = state_43171__$1;
(statearr_43177_43197[(1)] = (8));

} else {
var statearr_43178_43198 = state_43171__$1;
(statearr_43178_43198[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43172 === (3))){
var inst_43169 = (state_43171[(2)]);
var state_43171__$1 = state_43171;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_43171__$1,inst_43169);
} else {
if((state_val_43172 === (12))){
var state_43171__$1 = state_43171;
var statearr_43179_43199 = state_43171__$1;
(statearr_43179_43199[(2)] = null);

(statearr_43179_43199[(1)] = (13));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43172 === (2))){
var inst_43148 = (state_43171[(7)]);
var state_43171__$1 = state_43171;
if(cljs.core.truth_(inst_43148)){
var statearr_43180_43200 = state_43171__$1;
(statearr_43180_43200[(1)] = (4));

} else {
var statearr_43181_43201 = state_43171__$1;
(statearr_43181_43201[(1)] = (5));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43172 === (11))){
var inst_43162 = cljs.core.async.close_BANG_.call(null,ch);
var state_43171__$1 = state_43171;
var statearr_43182_43202 = state_43171__$1;
(statearr_43182_43202[(2)] = inst_43162);

(statearr_43182_43202[(1)] = (13));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43172 === (9))){
var state_43171__$1 = state_43171;
if(cljs.core.truth_(close_QMARK_)){
var statearr_43183_43203 = state_43171__$1;
(statearr_43183_43203[(1)] = (11));

} else {
var statearr_43184_43204 = state_43171__$1;
(statearr_43184_43204[(1)] = (12));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43172 === (5))){
var inst_43148 = (state_43171[(7)]);
var state_43171__$1 = state_43171;
var statearr_43185_43205 = state_43171__$1;
(statearr_43185_43205[(2)] = inst_43148);

(statearr_43185_43205[(1)] = (6));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43172 === (10))){
var inst_43167 = (state_43171[(2)]);
var state_43171__$1 = state_43171;
var statearr_43186_43206 = state_43171__$1;
(statearr_43186_43206[(2)] = inst_43167);

(statearr_43186_43206[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43172 === (8))){
var inst_43148 = (state_43171[(7)]);
var inst_43158 = cljs.core.next.call(null,inst_43148);
var inst_43148__$1 = inst_43158;
var state_43171__$1 = (function (){var statearr_43187 = state_43171;
(statearr_43187[(7)] = inst_43148__$1);

return statearr_43187;
})();
var statearr_43188_43207 = state_43171__$1;
(statearr_43188_43207[(2)] = null);

(statearr_43188_43207[(1)] = (2));


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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_43189 = [null,null,null,null,null,null,null,null];
(statearr_43189[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_43189[(1)] = (1));

return statearr_43189;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_43171){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_43171);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e43190){if((e43190 instanceof Object)){
var ex__26709__auto__ = e43190;
var statearr_43191_43208 = state_43171;
(statearr_43191_43208[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_43171);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e43190;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__43209 = state_43171;
state_43171 = G__43209;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_43171){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_43171);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_43192 = f__26801__auto__.call(null);
(statearr_43192[(6)] = c__26800__auto__);

return statearr_43192;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));

return c__26800__auto__;
}));

(cljs.core.async.onto_chan.cljs$lang$maxFixedArity = 3);

/**
 * Creates and returns a channel which contains the contents of coll,
 *   closing when exhausted.
 */
cljs.core.async.to_chan = (function cljs$core$async$to_chan(coll){
var ch = cljs.core.async.chan.call(null,cljs.core.bounded_count.call(null,(100),coll));
cljs.core.async.onto_chan.call(null,ch,coll);

return ch;
});

/**
 * @interface
 */
cljs.core.async.Mux = function(){};

var cljs$core$async$Mux$muxch_STAR_$dyn_43210 = (function (_){
var x__4428__auto__ = (((_ == null))?null:_);
var m__4429__auto__ = (cljs.core.async.muxch_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,_);
} else {
var m__4426__auto__ = (cljs.core.async.muxch_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,_);
} else {
throw cljs.core.missing_protocol.call(null,"Mux.muxch*",_);
}
}
});
cljs.core.async.muxch_STAR_ = (function cljs$core$async$muxch_STAR_(_){
if((((!((_ == null)))) && ((!((_.cljs$core$async$Mux$muxch_STAR_$arity$1 == null)))))){
return _.cljs$core$async$Mux$muxch_STAR_$arity$1(_);
} else {
return cljs$core$async$Mux$muxch_STAR_$dyn_43210.call(null,_);
}
});


/**
 * @interface
 */
cljs.core.async.Mult = function(){};

var cljs$core$async$Mult$tap_STAR_$dyn_43211 = (function (m,ch,close_QMARK_){
var x__4428__auto__ = (((m == null))?null:m);
var m__4429__auto__ = (cljs.core.async.tap_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,m,ch,close_QMARK_);
} else {
var m__4426__auto__ = (cljs.core.async.tap_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,m,ch,close_QMARK_);
} else {
throw cljs.core.missing_protocol.call(null,"Mult.tap*",m);
}
}
});
cljs.core.async.tap_STAR_ = (function cljs$core$async$tap_STAR_(m,ch,close_QMARK_){
if((((!((m == null)))) && ((!((m.cljs$core$async$Mult$tap_STAR_$arity$3 == null)))))){
return m.cljs$core$async$Mult$tap_STAR_$arity$3(m,ch,close_QMARK_);
} else {
return cljs$core$async$Mult$tap_STAR_$dyn_43211.call(null,m,ch,close_QMARK_);
}
});

var cljs$core$async$Mult$untap_STAR_$dyn_43212 = (function (m,ch){
var x__4428__auto__ = (((m == null))?null:m);
var m__4429__auto__ = (cljs.core.async.untap_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,m,ch);
} else {
var m__4426__auto__ = (cljs.core.async.untap_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,m,ch);
} else {
throw cljs.core.missing_protocol.call(null,"Mult.untap*",m);
}
}
});
cljs.core.async.untap_STAR_ = (function cljs$core$async$untap_STAR_(m,ch){
if((((!((m == null)))) && ((!((m.cljs$core$async$Mult$untap_STAR_$arity$2 == null)))))){
return m.cljs$core$async$Mult$untap_STAR_$arity$2(m,ch);
} else {
return cljs$core$async$Mult$untap_STAR_$dyn_43212.call(null,m,ch);
}
});

var cljs$core$async$Mult$untap_all_STAR_$dyn_43213 = (function (m){
var x__4428__auto__ = (((m == null))?null:m);
var m__4429__auto__ = (cljs.core.async.untap_all_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,m);
} else {
var m__4426__auto__ = (cljs.core.async.untap_all_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,m);
} else {
throw cljs.core.missing_protocol.call(null,"Mult.untap-all*",m);
}
}
});
cljs.core.async.untap_all_STAR_ = (function cljs$core$async$untap_all_STAR_(m){
if((((!((m == null)))) && ((!((m.cljs$core$async$Mult$untap_all_STAR_$arity$1 == null)))))){
return m.cljs$core$async$Mult$untap_all_STAR_$arity$1(m);
} else {
return cljs$core$async$Mult$untap_all_STAR_$dyn_43213.call(null,m);
}
});

/**
 * Creates and returns a mult(iple) of the supplied channel. Channels
 *   containing copies of the channel can be created with 'tap', and
 *   detached with 'untap'.
 * 
 *   Each item is distributed to all taps in parallel and synchronously,
 *   i.e. each tap must accept before the next item is distributed. Use
 *   buffering/windowing to prevent slow taps from holding up the mult.
 * 
 *   Items received when there are no taps get dropped.
 * 
 *   If a tap puts to a closed channel, it will be removed from the mult.
 */
cljs.core.async.mult = (function cljs$core$async$mult(ch){
var cs = cljs.core.atom.call(null,cljs.core.PersistentArrayMap.EMPTY);
var m = (function (){
if((typeof cljs !== 'undefined') && (typeof cljs.core !== 'undefined') && (typeof cljs.core.async !== 'undefined') && (typeof cljs.core.async.t_cljs$core$async43214 !== 'undefined')){
} else {

/**
* @constructor
 * @implements {cljs.core.async.Mult}
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.async.Mux}
 * @implements {cljs.core.IWithMeta}
*/
cljs.core.async.t_cljs$core$async43214 = (function (ch,cs,meta43215){
this.ch = ch;
this.cs = cs;
this.meta43215 = meta43215;
this.cljs$lang$protocol_mask$partition0$ = 393216;
this.cljs$lang$protocol_mask$partition1$ = 0;
});
(cljs.core.async.t_cljs$core$async43214.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (_43216,meta43215__$1){
var self__ = this;
var _43216__$1 = this;
return (new cljs.core.async.t_cljs$core$async43214(self__.ch,self__.cs,meta43215__$1));
}));

(cljs.core.async.t_cljs$core$async43214.prototype.cljs$core$IMeta$_meta$arity$1 = (function (_43216){
var self__ = this;
var _43216__$1 = this;
return self__.meta43215;
}));

(cljs.core.async.t_cljs$core$async43214.prototype.cljs$core$async$Mux$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async43214.prototype.cljs$core$async$Mux$muxch_STAR_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return self__.ch;
}));

(cljs.core.async.t_cljs$core$async43214.prototype.cljs$core$async$Mult$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async43214.prototype.cljs$core$async$Mult$tap_STAR_$arity$3 = (function (_,ch__$1,close_QMARK_){
var self__ = this;
var ___$1 = this;
cljs.core.swap_BANG_.call(null,self__.cs,cljs.core.assoc,ch__$1,close_QMARK_);

return null;
}));

(cljs.core.async.t_cljs$core$async43214.prototype.cljs$core$async$Mult$untap_STAR_$arity$2 = (function (_,ch__$1){
var self__ = this;
var ___$1 = this;
cljs.core.swap_BANG_.call(null,self__.cs,cljs.core.dissoc,ch__$1);

return null;
}));

(cljs.core.async.t_cljs$core$async43214.prototype.cljs$core$async$Mult$untap_all_STAR_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
cljs.core.reset_BANG_.call(null,self__.cs,cljs.core.PersistentArrayMap.EMPTY);

return null;
}));

(cljs.core.async.t_cljs$core$async43214.getBasis = (function (){
return new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"ch","ch",1085813622,null),new cljs.core.Symbol(null,"cs","cs",-117024463,null),new cljs.core.Symbol(null,"meta43215","meta43215",1301707768,null)], null);
}));

(cljs.core.async.t_cljs$core$async43214.cljs$lang$type = true);

(cljs.core.async.t_cljs$core$async43214.cljs$lang$ctorStr = "cljs.core.async/t_cljs$core$async43214");

(cljs.core.async.t_cljs$core$async43214.cljs$lang$ctorPrWriter = (function (this__4369__auto__,writer__4370__auto__,opt__4371__auto__){
return cljs.core._write.call(null,writer__4370__auto__,"cljs.core.async/t_cljs$core$async43214");
}));

/**
 * Positional factory function for cljs.core.async/t_cljs$core$async43214.
 */
cljs.core.async.__GT_t_cljs$core$async43214 = (function cljs$core$async$mult_$___GT_t_cljs$core$async43214(ch__$1,cs__$1,meta43215){
return (new cljs.core.async.t_cljs$core$async43214(ch__$1,cs__$1,meta43215));
});

}

return (new cljs.core.async.t_cljs$core$async43214(ch,cs,cljs.core.PersistentArrayMap.EMPTY));
})()
;
var dchan = cljs.core.async.chan.call(null,(1));
var dctr = cljs.core.atom.call(null,null);
var done = (function (_){
if((cljs.core.swap_BANG_.call(null,dctr,cljs.core.dec) === (0))){
return cljs.core.async.put_BANG_.call(null,dchan,true);
} else {
return null;
}
});
var c__26800__auto___43436 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_43351){
var state_val_43352 = (state_43351[(1)]);
if((state_val_43352 === (7))){
var inst_43347 = (state_43351[(2)]);
var state_43351__$1 = state_43351;
var statearr_43353_43437 = state_43351__$1;
(statearr_43353_43437[(2)] = inst_43347);

(statearr_43353_43437[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (20))){
var inst_43250 = (state_43351[(7)]);
var inst_43262 = cljs.core.first.call(null,inst_43250);
var inst_43263 = cljs.core.nth.call(null,inst_43262,(0),null);
var inst_43264 = cljs.core.nth.call(null,inst_43262,(1),null);
var state_43351__$1 = (function (){var statearr_43354 = state_43351;
(statearr_43354[(8)] = inst_43263);

return statearr_43354;
})();
if(cljs.core.truth_(inst_43264)){
var statearr_43355_43438 = state_43351__$1;
(statearr_43355_43438[(1)] = (22));

} else {
var statearr_43356_43439 = state_43351__$1;
(statearr_43356_43439[(1)] = (23));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (27))){
var inst_43299 = (state_43351[(9)]);
var inst_43219 = (state_43351[(10)]);
var inst_43292 = (state_43351[(11)]);
var inst_43294 = (state_43351[(12)]);
var inst_43299__$1 = cljs.core._nth.call(null,inst_43292,inst_43294);
var inst_43300 = cljs.core.async.put_BANG_.call(null,inst_43299__$1,inst_43219,done);
var state_43351__$1 = (function (){var statearr_43357 = state_43351;
(statearr_43357[(9)] = inst_43299__$1);

return statearr_43357;
})();
if(cljs.core.truth_(inst_43300)){
var statearr_43358_43440 = state_43351__$1;
(statearr_43358_43440[(1)] = (30));

} else {
var statearr_43359_43441 = state_43351__$1;
(statearr_43359_43441[(1)] = (31));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (1))){
var state_43351__$1 = state_43351;
var statearr_43360_43442 = state_43351__$1;
(statearr_43360_43442[(2)] = null);

(statearr_43360_43442[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (24))){
var inst_43250 = (state_43351[(7)]);
var inst_43269 = (state_43351[(2)]);
var inst_43270 = cljs.core.next.call(null,inst_43250);
var inst_43228 = inst_43270;
var inst_43229 = null;
var inst_43230 = (0);
var inst_43231 = (0);
var state_43351__$1 = (function (){var statearr_43361 = state_43351;
(statearr_43361[(13)] = inst_43269);

(statearr_43361[(14)] = inst_43228);

(statearr_43361[(15)] = inst_43230);

(statearr_43361[(16)] = inst_43231);

(statearr_43361[(17)] = inst_43229);

return statearr_43361;
})();
var statearr_43362_43443 = state_43351__$1;
(statearr_43362_43443[(2)] = null);

(statearr_43362_43443[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (39))){
var state_43351__$1 = state_43351;
var statearr_43366_43444 = state_43351__$1;
(statearr_43366_43444[(2)] = null);

(statearr_43366_43444[(1)] = (41));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (4))){
var inst_43219 = (state_43351[(10)]);
var inst_43219__$1 = (state_43351[(2)]);
var inst_43220 = (inst_43219__$1 == null);
var state_43351__$1 = (function (){var statearr_43367 = state_43351;
(statearr_43367[(10)] = inst_43219__$1);

return statearr_43367;
})();
if(cljs.core.truth_(inst_43220)){
var statearr_43368_43445 = state_43351__$1;
(statearr_43368_43445[(1)] = (5));

} else {
var statearr_43369_43446 = state_43351__$1;
(statearr_43369_43446[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (15))){
var inst_43228 = (state_43351[(14)]);
var inst_43230 = (state_43351[(15)]);
var inst_43231 = (state_43351[(16)]);
var inst_43229 = (state_43351[(17)]);
var inst_43246 = (state_43351[(2)]);
var inst_43247 = (inst_43231 + (1));
var tmp43363 = inst_43228;
var tmp43364 = inst_43230;
var tmp43365 = inst_43229;
var inst_43228__$1 = tmp43363;
var inst_43229__$1 = tmp43365;
var inst_43230__$1 = tmp43364;
var inst_43231__$1 = inst_43247;
var state_43351__$1 = (function (){var statearr_43370 = state_43351;
(statearr_43370[(18)] = inst_43246);

(statearr_43370[(14)] = inst_43228__$1);

(statearr_43370[(15)] = inst_43230__$1);

(statearr_43370[(16)] = inst_43231__$1);

(statearr_43370[(17)] = inst_43229__$1);

return statearr_43370;
})();
var statearr_43371_43447 = state_43351__$1;
(statearr_43371_43447[(2)] = null);

(statearr_43371_43447[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (21))){
var inst_43273 = (state_43351[(2)]);
var state_43351__$1 = state_43351;
var statearr_43375_43448 = state_43351__$1;
(statearr_43375_43448[(2)] = inst_43273);

(statearr_43375_43448[(1)] = (18));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (31))){
var inst_43299 = (state_43351[(9)]);
var inst_43303 = done.call(null,null);
var inst_43304 = cljs.core.async.untap_STAR_.call(null,m,inst_43299);
var state_43351__$1 = (function (){var statearr_43376 = state_43351;
(statearr_43376[(19)] = inst_43303);

return statearr_43376;
})();
var statearr_43377_43449 = state_43351__$1;
(statearr_43377_43449[(2)] = inst_43304);

(statearr_43377_43449[(1)] = (32));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (32))){
var inst_43293 = (state_43351[(20)]);
var inst_43292 = (state_43351[(11)]);
var inst_43294 = (state_43351[(12)]);
var inst_43291 = (state_43351[(21)]);
var inst_43306 = (state_43351[(2)]);
var inst_43307 = (inst_43294 + (1));
var tmp43372 = inst_43293;
var tmp43373 = inst_43292;
var tmp43374 = inst_43291;
var inst_43291__$1 = tmp43374;
var inst_43292__$1 = tmp43373;
var inst_43293__$1 = tmp43372;
var inst_43294__$1 = inst_43307;
var state_43351__$1 = (function (){var statearr_43378 = state_43351;
(statearr_43378[(20)] = inst_43293__$1);

(statearr_43378[(22)] = inst_43306);

(statearr_43378[(11)] = inst_43292__$1);

(statearr_43378[(12)] = inst_43294__$1);

(statearr_43378[(21)] = inst_43291__$1);

return statearr_43378;
})();
var statearr_43379_43450 = state_43351__$1;
(statearr_43379_43450[(2)] = null);

(statearr_43379_43450[(1)] = (25));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (40))){
var inst_43319 = (state_43351[(23)]);
var inst_43323 = done.call(null,null);
var inst_43324 = cljs.core.async.untap_STAR_.call(null,m,inst_43319);
var state_43351__$1 = (function (){var statearr_43380 = state_43351;
(statearr_43380[(24)] = inst_43323);

return statearr_43380;
})();
var statearr_43381_43451 = state_43351__$1;
(statearr_43381_43451[(2)] = inst_43324);

(statearr_43381_43451[(1)] = (41));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (33))){
var inst_43310 = (state_43351[(25)]);
var inst_43312 = cljs.core.chunked_seq_QMARK_.call(null,inst_43310);
var state_43351__$1 = state_43351;
if(inst_43312){
var statearr_43382_43452 = state_43351__$1;
(statearr_43382_43452[(1)] = (36));

} else {
var statearr_43383_43453 = state_43351__$1;
(statearr_43383_43453[(1)] = (37));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (13))){
var inst_43240 = (state_43351[(26)]);
var inst_43243 = cljs.core.async.close_BANG_.call(null,inst_43240);
var state_43351__$1 = state_43351;
var statearr_43384_43454 = state_43351__$1;
(statearr_43384_43454[(2)] = inst_43243);

(statearr_43384_43454[(1)] = (15));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (22))){
var inst_43263 = (state_43351[(8)]);
var inst_43266 = cljs.core.async.close_BANG_.call(null,inst_43263);
var state_43351__$1 = state_43351;
var statearr_43385_43455 = state_43351__$1;
(statearr_43385_43455[(2)] = inst_43266);

(statearr_43385_43455[(1)] = (24));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (36))){
var inst_43310 = (state_43351[(25)]);
var inst_43314 = cljs.core.chunk_first.call(null,inst_43310);
var inst_43315 = cljs.core.chunk_rest.call(null,inst_43310);
var inst_43316 = cljs.core.count.call(null,inst_43314);
var inst_43291 = inst_43315;
var inst_43292 = inst_43314;
var inst_43293 = inst_43316;
var inst_43294 = (0);
var state_43351__$1 = (function (){var statearr_43386 = state_43351;
(statearr_43386[(20)] = inst_43293);

(statearr_43386[(11)] = inst_43292);

(statearr_43386[(12)] = inst_43294);

(statearr_43386[(21)] = inst_43291);

return statearr_43386;
})();
var statearr_43387_43456 = state_43351__$1;
(statearr_43387_43456[(2)] = null);

(statearr_43387_43456[(1)] = (25));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (41))){
var inst_43310 = (state_43351[(25)]);
var inst_43326 = (state_43351[(2)]);
var inst_43327 = cljs.core.next.call(null,inst_43310);
var inst_43291 = inst_43327;
var inst_43292 = null;
var inst_43293 = (0);
var inst_43294 = (0);
var state_43351__$1 = (function (){var statearr_43388 = state_43351;
(statearr_43388[(20)] = inst_43293);

(statearr_43388[(27)] = inst_43326);

(statearr_43388[(11)] = inst_43292);

(statearr_43388[(12)] = inst_43294);

(statearr_43388[(21)] = inst_43291);

return statearr_43388;
})();
var statearr_43389_43457 = state_43351__$1;
(statearr_43389_43457[(2)] = null);

(statearr_43389_43457[(1)] = (25));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (43))){
var state_43351__$1 = state_43351;
var statearr_43390_43458 = state_43351__$1;
(statearr_43390_43458[(2)] = null);

(statearr_43390_43458[(1)] = (44));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (29))){
var inst_43335 = (state_43351[(2)]);
var state_43351__$1 = state_43351;
var statearr_43391_43459 = state_43351__$1;
(statearr_43391_43459[(2)] = inst_43335);

(statearr_43391_43459[(1)] = (26));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (44))){
var inst_43344 = (state_43351[(2)]);
var state_43351__$1 = (function (){var statearr_43392 = state_43351;
(statearr_43392[(28)] = inst_43344);

return statearr_43392;
})();
var statearr_43393_43460 = state_43351__$1;
(statearr_43393_43460[(2)] = null);

(statearr_43393_43460[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (6))){
var inst_43283 = (state_43351[(29)]);
var inst_43282 = cljs.core.deref.call(null,cs);
var inst_43283__$1 = cljs.core.keys.call(null,inst_43282);
var inst_43284 = cljs.core.count.call(null,inst_43283__$1);
var inst_43285 = cljs.core.reset_BANG_.call(null,dctr,inst_43284);
var inst_43290 = cljs.core.seq.call(null,inst_43283__$1);
var inst_43291 = inst_43290;
var inst_43292 = null;
var inst_43293 = (0);
var inst_43294 = (0);
var state_43351__$1 = (function (){var statearr_43394 = state_43351;
(statearr_43394[(20)] = inst_43293);

(statearr_43394[(30)] = inst_43285);

(statearr_43394[(29)] = inst_43283__$1);

(statearr_43394[(11)] = inst_43292);

(statearr_43394[(12)] = inst_43294);

(statearr_43394[(21)] = inst_43291);

return statearr_43394;
})();
var statearr_43395_43461 = state_43351__$1;
(statearr_43395_43461[(2)] = null);

(statearr_43395_43461[(1)] = (25));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (28))){
var inst_43310 = (state_43351[(25)]);
var inst_43291 = (state_43351[(21)]);
var inst_43310__$1 = cljs.core.seq.call(null,inst_43291);
var state_43351__$1 = (function (){var statearr_43396 = state_43351;
(statearr_43396[(25)] = inst_43310__$1);

return statearr_43396;
})();
if(inst_43310__$1){
var statearr_43397_43462 = state_43351__$1;
(statearr_43397_43462[(1)] = (33));

} else {
var statearr_43398_43463 = state_43351__$1;
(statearr_43398_43463[(1)] = (34));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (25))){
var inst_43293 = (state_43351[(20)]);
var inst_43294 = (state_43351[(12)]);
var inst_43296 = (inst_43294 < inst_43293);
var inst_43297 = inst_43296;
var state_43351__$1 = state_43351;
if(cljs.core.truth_(inst_43297)){
var statearr_43399_43464 = state_43351__$1;
(statearr_43399_43464[(1)] = (27));

} else {
var statearr_43400_43465 = state_43351__$1;
(statearr_43400_43465[(1)] = (28));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (34))){
var state_43351__$1 = state_43351;
var statearr_43401_43466 = state_43351__$1;
(statearr_43401_43466[(2)] = null);

(statearr_43401_43466[(1)] = (35));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (17))){
var state_43351__$1 = state_43351;
var statearr_43402_43467 = state_43351__$1;
(statearr_43402_43467[(2)] = null);

(statearr_43402_43467[(1)] = (18));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (3))){
var inst_43349 = (state_43351[(2)]);
var state_43351__$1 = state_43351;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_43351__$1,inst_43349);
} else {
if((state_val_43352 === (12))){
var inst_43278 = (state_43351[(2)]);
var state_43351__$1 = state_43351;
var statearr_43403_43468 = state_43351__$1;
(statearr_43403_43468[(2)] = inst_43278);

(statearr_43403_43468[(1)] = (9));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (2))){
var state_43351__$1 = state_43351;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_43351__$1,(4),ch);
} else {
if((state_val_43352 === (23))){
var state_43351__$1 = state_43351;
var statearr_43404_43469 = state_43351__$1;
(statearr_43404_43469[(2)] = null);

(statearr_43404_43469[(1)] = (24));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (35))){
var inst_43333 = (state_43351[(2)]);
var state_43351__$1 = state_43351;
var statearr_43405_43470 = state_43351__$1;
(statearr_43405_43470[(2)] = inst_43333);

(statearr_43405_43470[(1)] = (29));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (19))){
var inst_43250 = (state_43351[(7)]);
var inst_43254 = cljs.core.chunk_first.call(null,inst_43250);
var inst_43255 = cljs.core.chunk_rest.call(null,inst_43250);
var inst_43256 = cljs.core.count.call(null,inst_43254);
var inst_43228 = inst_43255;
var inst_43229 = inst_43254;
var inst_43230 = inst_43256;
var inst_43231 = (0);
var state_43351__$1 = (function (){var statearr_43406 = state_43351;
(statearr_43406[(14)] = inst_43228);

(statearr_43406[(15)] = inst_43230);

(statearr_43406[(16)] = inst_43231);

(statearr_43406[(17)] = inst_43229);

return statearr_43406;
})();
var statearr_43407_43471 = state_43351__$1;
(statearr_43407_43471[(2)] = null);

(statearr_43407_43471[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (11))){
var inst_43250 = (state_43351[(7)]);
var inst_43228 = (state_43351[(14)]);
var inst_43250__$1 = cljs.core.seq.call(null,inst_43228);
var state_43351__$1 = (function (){var statearr_43408 = state_43351;
(statearr_43408[(7)] = inst_43250__$1);

return statearr_43408;
})();
if(inst_43250__$1){
var statearr_43409_43472 = state_43351__$1;
(statearr_43409_43472[(1)] = (16));

} else {
var statearr_43410_43473 = state_43351__$1;
(statearr_43410_43473[(1)] = (17));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (9))){
var inst_43280 = (state_43351[(2)]);
var state_43351__$1 = state_43351;
var statearr_43411_43474 = state_43351__$1;
(statearr_43411_43474[(2)] = inst_43280);

(statearr_43411_43474[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (5))){
var inst_43226 = cljs.core.deref.call(null,cs);
var inst_43227 = cljs.core.seq.call(null,inst_43226);
var inst_43228 = inst_43227;
var inst_43229 = null;
var inst_43230 = (0);
var inst_43231 = (0);
var state_43351__$1 = (function (){var statearr_43412 = state_43351;
(statearr_43412[(14)] = inst_43228);

(statearr_43412[(15)] = inst_43230);

(statearr_43412[(16)] = inst_43231);

(statearr_43412[(17)] = inst_43229);

return statearr_43412;
})();
var statearr_43413_43475 = state_43351__$1;
(statearr_43413_43475[(2)] = null);

(statearr_43413_43475[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (14))){
var state_43351__$1 = state_43351;
var statearr_43414_43476 = state_43351__$1;
(statearr_43414_43476[(2)] = null);

(statearr_43414_43476[(1)] = (15));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (45))){
var inst_43341 = (state_43351[(2)]);
var state_43351__$1 = state_43351;
var statearr_43415_43477 = state_43351__$1;
(statearr_43415_43477[(2)] = inst_43341);

(statearr_43415_43477[(1)] = (44));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (26))){
var inst_43283 = (state_43351[(29)]);
var inst_43337 = (state_43351[(2)]);
var inst_43338 = cljs.core.seq.call(null,inst_43283);
var state_43351__$1 = (function (){var statearr_43416 = state_43351;
(statearr_43416[(31)] = inst_43337);

return statearr_43416;
})();
if(inst_43338){
var statearr_43417_43478 = state_43351__$1;
(statearr_43417_43478[(1)] = (42));

} else {
var statearr_43418_43479 = state_43351__$1;
(statearr_43418_43479[(1)] = (43));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (16))){
var inst_43250 = (state_43351[(7)]);
var inst_43252 = cljs.core.chunked_seq_QMARK_.call(null,inst_43250);
var state_43351__$1 = state_43351;
if(inst_43252){
var statearr_43419_43480 = state_43351__$1;
(statearr_43419_43480[(1)] = (19));

} else {
var statearr_43420_43481 = state_43351__$1;
(statearr_43420_43481[(1)] = (20));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (38))){
var inst_43330 = (state_43351[(2)]);
var state_43351__$1 = state_43351;
var statearr_43421_43482 = state_43351__$1;
(statearr_43421_43482[(2)] = inst_43330);

(statearr_43421_43482[(1)] = (35));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (30))){
var state_43351__$1 = state_43351;
var statearr_43422_43483 = state_43351__$1;
(statearr_43422_43483[(2)] = null);

(statearr_43422_43483[(1)] = (32));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (10))){
var inst_43231 = (state_43351[(16)]);
var inst_43229 = (state_43351[(17)]);
var inst_43239 = cljs.core._nth.call(null,inst_43229,inst_43231);
var inst_43240 = cljs.core.nth.call(null,inst_43239,(0),null);
var inst_43241 = cljs.core.nth.call(null,inst_43239,(1),null);
var state_43351__$1 = (function (){var statearr_43423 = state_43351;
(statearr_43423[(26)] = inst_43240);

return statearr_43423;
})();
if(cljs.core.truth_(inst_43241)){
var statearr_43424_43484 = state_43351__$1;
(statearr_43424_43484[(1)] = (13));

} else {
var statearr_43425_43485 = state_43351__$1;
(statearr_43425_43485[(1)] = (14));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (18))){
var inst_43276 = (state_43351[(2)]);
var state_43351__$1 = state_43351;
var statearr_43426_43486 = state_43351__$1;
(statearr_43426_43486[(2)] = inst_43276);

(statearr_43426_43486[(1)] = (12));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (42))){
var state_43351__$1 = state_43351;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_43351__$1,(45),dchan);
} else {
if((state_val_43352 === (37))){
var inst_43310 = (state_43351[(25)]);
var inst_43219 = (state_43351[(10)]);
var inst_43319 = (state_43351[(23)]);
var inst_43319__$1 = cljs.core.first.call(null,inst_43310);
var inst_43320 = cljs.core.async.put_BANG_.call(null,inst_43319__$1,inst_43219,done);
var state_43351__$1 = (function (){var statearr_43427 = state_43351;
(statearr_43427[(23)] = inst_43319__$1);

return statearr_43427;
})();
if(cljs.core.truth_(inst_43320)){
var statearr_43428_43487 = state_43351__$1;
(statearr_43428_43487[(1)] = (39));

} else {
var statearr_43429_43488 = state_43351__$1;
(statearr_43429_43488[(1)] = (40));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43352 === (8))){
var inst_43230 = (state_43351[(15)]);
var inst_43231 = (state_43351[(16)]);
var inst_43233 = (inst_43231 < inst_43230);
var inst_43234 = inst_43233;
var state_43351__$1 = state_43351;
if(cljs.core.truth_(inst_43234)){
var statearr_43430_43489 = state_43351__$1;
(statearr_43430_43489[(1)] = (10));

} else {
var statearr_43431_43490 = state_43351__$1;
(statearr_43431_43490[(1)] = (11));

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
});
return (function() {
var cljs$core$async$mult_$_state_machine__26706__auto__ = null;
var cljs$core$async$mult_$_state_machine__26706__auto____0 = (function (){
var statearr_43432 = [null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_43432[(0)] = cljs$core$async$mult_$_state_machine__26706__auto__);

(statearr_43432[(1)] = (1));

return statearr_43432;
});
var cljs$core$async$mult_$_state_machine__26706__auto____1 = (function (state_43351){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_43351);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e43433){if((e43433 instanceof Object)){
var ex__26709__auto__ = e43433;
var statearr_43434_43491 = state_43351;
(statearr_43434_43491[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_43351);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e43433;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__43492 = state_43351;
state_43351 = G__43492;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$mult_$_state_machine__26706__auto__ = function(state_43351){
switch(arguments.length){
case 0:
return cljs$core$async$mult_$_state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$mult_$_state_machine__26706__auto____1.call(this,state_43351);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$mult_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$mult_$_state_machine__26706__auto____0;
cljs$core$async$mult_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$mult_$_state_machine__26706__auto____1;
return cljs$core$async$mult_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_43435 = f__26801__auto__.call(null);
(statearr_43435[(6)] = c__26800__auto___43436);

return statearr_43435;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return m;
});
/**
 * Copies the mult source onto the supplied channel.
 * 
 *   By default the channel will be closed when the source closes,
 *   but can be determined by the close? parameter.
 */
cljs.core.async.tap = (function cljs$core$async$tap(var_args){
var G__43494 = arguments.length;
switch (G__43494) {
case 2:
return cljs.core.async.tap.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.tap.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.tap.cljs$core$IFn$_invoke$arity$2 = (function (mult,ch){
return cljs.core.async.tap.call(null,mult,ch,true);
}));

(cljs.core.async.tap.cljs$core$IFn$_invoke$arity$3 = (function (mult,ch,close_QMARK_){
cljs.core.async.tap_STAR_.call(null,mult,ch,close_QMARK_);

return ch;
}));

(cljs.core.async.tap.cljs$lang$maxFixedArity = 3);

/**
 * Disconnects a target channel from a mult
 */
cljs.core.async.untap = (function cljs$core$async$untap(mult,ch){
return cljs.core.async.untap_STAR_.call(null,mult,ch);
});
/**
 * Disconnects all target channels from a mult
 */
cljs.core.async.untap_all = (function cljs$core$async$untap_all(mult){
return cljs.core.async.untap_all_STAR_.call(null,mult);
});

/**
 * @interface
 */
cljs.core.async.Mix = function(){};

var cljs$core$async$Mix$admix_STAR_$dyn_43496 = (function (m,ch){
var x__4428__auto__ = (((m == null))?null:m);
var m__4429__auto__ = (cljs.core.async.admix_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,m,ch);
} else {
var m__4426__auto__ = (cljs.core.async.admix_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,m,ch);
} else {
throw cljs.core.missing_protocol.call(null,"Mix.admix*",m);
}
}
});
cljs.core.async.admix_STAR_ = (function cljs$core$async$admix_STAR_(m,ch){
if((((!((m == null)))) && ((!((m.cljs$core$async$Mix$admix_STAR_$arity$2 == null)))))){
return m.cljs$core$async$Mix$admix_STAR_$arity$2(m,ch);
} else {
return cljs$core$async$Mix$admix_STAR_$dyn_43496.call(null,m,ch);
}
});

var cljs$core$async$Mix$unmix_STAR_$dyn_43497 = (function (m,ch){
var x__4428__auto__ = (((m == null))?null:m);
var m__4429__auto__ = (cljs.core.async.unmix_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,m,ch);
} else {
var m__4426__auto__ = (cljs.core.async.unmix_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,m,ch);
} else {
throw cljs.core.missing_protocol.call(null,"Mix.unmix*",m);
}
}
});
cljs.core.async.unmix_STAR_ = (function cljs$core$async$unmix_STAR_(m,ch){
if((((!((m == null)))) && ((!((m.cljs$core$async$Mix$unmix_STAR_$arity$2 == null)))))){
return m.cljs$core$async$Mix$unmix_STAR_$arity$2(m,ch);
} else {
return cljs$core$async$Mix$unmix_STAR_$dyn_43497.call(null,m,ch);
}
});

var cljs$core$async$Mix$unmix_all_STAR_$dyn_43498 = (function (m){
var x__4428__auto__ = (((m == null))?null:m);
var m__4429__auto__ = (cljs.core.async.unmix_all_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,m);
} else {
var m__4426__auto__ = (cljs.core.async.unmix_all_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,m);
} else {
throw cljs.core.missing_protocol.call(null,"Mix.unmix-all*",m);
}
}
});
cljs.core.async.unmix_all_STAR_ = (function cljs$core$async$unmix_all_STAR_(m){
if((((!((m == null)))) && ((!((m.cljs$core$async$Mix$unmix_all_STAR_$arity$1 == null)))))){
return m.cljs$core$async$Mix$unmix_all_STAR_$arity$1(m);
} else {
return cljs$core$async$Mix$unmix_all_STAR_$dyn_43498.call(null,m);
}
});

var cljs$core$async$Mix$toggle_STAR_$dyn_43499 = (function (m,state_map){
var x__4428__auto__ = (((m == null))?null:m);
var m__4429__auto__ = (cljs.core.async.toggle_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,m,state_map);
} else {
var m__4426__auto__ = (cljs.core.async.toggle_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,m,state_map);
} else {
throw cljs.core.missing_protocol.call(null,"Mix.toggle*",m);
}
}
});
cljs.core.async.toggle_STAR_ = (function cljs$core$async$toggle_STAR_(m,state_map){
if((((!((m == null)))) && ((!((m.cljs$core$async$Mix$toggle_STAR_$arity$2 == null)))))){
return m.cljs$core$async$Mix$toggle_STAR_$arity$2(m,state_map);
} else {
return cljs$core$async$Mix$toggle_STAR_$dyn_43499.call(null,m,state_map);
}
});

var cljs$core$async$Mix$solo_mode_STAR_$dyn_43500 = (function (m,mode){
var x__4428__auto__ = (((m == null))?null:m);
var m__4429__auto__ = (cljs.core.async.solo_mode_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,m,mode);
} else {
var m__4426__auto__ = (cljs.core.async.solo_mode_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,m,mode);
} else {
throw cljs.core.missing_protocol.call(null,"Mix.solo-mode*",m);
}
}
});
cljs.core.async.solo_mode_STAR_ = (function cljs$core$async$solo_mode_STAR_(m,mode){
if((((!((m == null)))) && ((!((m.cljs$core$async$Mix$solo_mode_STAR_$arity$2 == null)))))){
return m.cljs$core$async$Mix$solo_mode_STAR_$arity$2(m,mode);
} else {
return cljs$core$async$Mix$solo_mode_STAR_$dyn_43500.call(null,m,mode);
}
});

cljs.core.async.ioc_alts_BANG_ = (function cljs$core$async$ioc_alts_BANG_(var_args){
var args__4742__auto__ = [];
var len__4736__auto___43511 = arguments.length;
var i__4737__auto___43512 = (0);
while(true){
if((i__4737__auto___43512 < len__4736__auto___43511)){
args__4742__auto__.push((arguments[i__4737__auto___43512]));

var G__43513 = (i__4737__auto___43512 + (1));
i__4737__auto___43512 = G__43513;
continue;
} else {
}
break;
}

var argseq__4743__auto__ = ((((3) < args__4742__auto__.length))?(new cljs.core.IndexedSeq(args__4742__auto__.slice((3)),(0),null)):null);
return cljs.core.async.ioc_alts_BANG_.cljs$core$IFn$_invoke$arity$variadic((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),argseq__4743__auto__);
});

(cljs.core.async.ioc_alts_BANG_.cljs$core$IFn$_invoke$arity$variadic = (function (state,cont_block,ports,p__43505){
var map__43506 = p__43505;
var map__43506__$1 = (((((!((map__43506 == null))))?(((((map__43506.cljs$lang$protocol_mask$partition0$ & (64))) || ((cljs.core.PROTOCOL_SENTINEL === map__43506.cljs$core$ISeq$))))?true:false):false))?cljs.core.apply.call(null,cljs.core.hash_map,map__43506):map__43506);
var opts = map__43506__$1;
var statearr_43508_43514 = state;
(statearr_43508_43514[(1)] = cont_block);


var temp__5720__auto__ = cljs.core.async.do_alts.call(null,(function (val){
var statearr_43509_43515 = state;
(statearr_43509_43515[(2)] = val);


return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state);
}),ports,opts);
if(cljs.core.truth_(temp__5720__auto__)){
var cb = temp__5720__auto__;
var statearr_43510_43516 = state;
(statearr_43510_43516[(2)] = cljs.core.deref.call(null,cb));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
return null;
}
}));

(cljs.core.async.ioc_alts_BANG_.cljs$lang$maxFixedArity = (3));

/** @this {Function} */
(cljs.core.async.ioc_alts_BANG_.cljs$lang$applyTo = (function (seq43501){
var G__43502 = cljs.core.first.call(null,seq43501);
var seq43501__$1 = cljs.core.next.call(null,seq43501);
var G__43503 = cljs.core.first.call(null,seq43501__$1);
var seq43501__$2 = cljs.core.next.call(null,seq43501__$1);
var G__43504 = cljs.core.first.call(null,seq43501__$2);
var seq43501__$3 = cljs.core.next.call(null,seq43501__$2);
var self__4723__auto__ = this;
return self__4723__auto__.cljs$core$IFn$_invoke$arity$variadic(G__43502,G__43503,G__43504,seq43501__$3);
}));

/**
 * Creates and returns a mix of one or more input channels which will
 *   be put on the supplied out channel. Input sources can be added to
 *   the mix with 'admix', and removed with 'unmix'. A mix supports
 *   soloing, muting and pausing multiple inputs atomically using
 *   'toggle', and can solo using either muting or pausing as determined
 *   by 'solo-mode'.
 * 
 *   Each channel can have zero or more boolean modes set via 'toggle':
 * 
 *   :solo - when true, only this (ond other soloed) channel(s) will appear
 *        in the mix output channel. :mute and :pause states of soloed
 *        channels are ignored. If solo-mode is :mute, non-soloed
 *        channels are muted, if :pause, non-soloed channels are
 *        paused.
 * 
 *   :mute - muted channels will have their contents consumed but not included in the mix
 *   :pause - paused channels will not have their contents consumed (and thus also not included in the mix)
 */
cljs.core.async.mix = (function cljs$core$async$mix(out){
var cs = cljs.core.atom.call(null,cljs.core.PersistentArrayMap.EMPTY);
var solo_modes = new cljs.core.PersistentHashSet(null, new cljs.core.PersistentArrayMap(null, 2, [new cljs.core.Keyword(null,"pause","pause",-2095325672),null,new cljs.core.Keyword(null,"mute","mute",1151223646),null], null), null);
var attrs = cljs.core.conj.call(null,solo_modes,new cljs.core.Keyword(null,"solo","solo",-316350075));
var solo_mode = cljs.core.atom.call(null,new cljs.core.Keyword(null,"mute","mute",1151223646));
var change = cljs.core.async.chan.call(null);
var changed = (function (){
return cljs.core.async.put_BANG_.call(null,change,true);
});
var pick = (function (attr,chs){
return cljs.core.reduce_kv.call(null,(function (ret,c,v){
if(cljs.core.truth_(attr.call(null,v))){
return cljs.core.conj.call(null,ret,c);
} else {
return ret;
}
}),cljs.core.PersistentHashSet.EMPTY,chs);
});
var calc_state = (function (){
var chs = cljs.core.deref.call(null,cs);
var mode = cljs.core.deref.call(null,solo_mode);
var solos = pick.call(null,new cljs.core.Keyword(null,"solo","solo",-316350075),chs);
var pauses = pick.call(null,new cljs.core.Keyword(null,"pause","pause",-2095325672),chs);
return new cljs.core.PersistentArrayMap(null, 3, [new cljs.core.Keyword(null,"solos","solos",1441458643),solos,new cljs.core.Keyword(null,"mutes","mutes",1068806309),pick.call(null,new cljs.core.Keyword(null,"mute","mute",1151223646),chs),new cljs.core.Keyword(null,"reads","reads",-1215067361),cljs.core.conj.call(null,((((cljs.core._EQ_.call(null,mode,new cljs.core.Keyword(null,"pause","pause",-2095325672))) && ((!(cljs.core.empty_QMARK_.call(null,solos))))))?cljs.core.vec.call(null,solos):cljs.core.vec.call(null,cljs.core.remove.call(null,pauses,cljs.core.keys.call(null,chs)))),change)], null);
});
var m = (function (){
if((typeof cljs !== 'undefined') && (typeof cljs.core !== 'undefined') && (typeof cljs.core.async !== 'undefined') && (typeof cljs.core.async.t_cljs$core$async43517 !== 'undefined')){
} else {

/**
* @constructor
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.async.Mix}
 * @implements {cljs.core.async.Mux}
 * @implements {cljs.core.IWithMeta}
*/
cljs.core.async.t_cljs$core$async43517 = (function (change,solo_mode,pick,cs,calc_state,out,changed,solo_modes,attrs,meta43518){
this.change = change;
this.solo_mode = solo_mode;
this.pick = pick;
this.cs = cs;
this.calc_state = calc_state;
this.out = out;
this.changed = changed;
this.solo_modes = solo_modes;
this.attrs = attrs;
this.meta43518 = meta43518;
this.cljs$lang$protocol_mask$partition0$ = 393216;
this.cljs$lang$protocol_mask$partition1$ = 0;
});
(cljs.core.async.t_cljs$core$async43517.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (_43519,meta43518__$1){
var self__ = this;
var _43519__$1 = this;
return (new cljs.core.async.t_cljs$core$async43517(self__.change,self__.solo_mode,self__.pick,self__.cs,self__.calc_state,self__.out,self__.changed,self__.solo_modes,self__.attrs,meta43518__$1));
}));

(cljs.core.async.t_cljs$core$async43517.prototype.cljs$core$IMeta$_meta$arity$1 = (function (_43519){
var self__ = this;
var _43519__$1 = this;
return self__.meta43518;
}));

(cljs.core.async.t_cljs$core$async43517.prototype.cljs$core$async$Mux$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async43517.prototype.cljs$core$async$Mux$muxch_STAR_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return self__.out;
}));

(cljs.core.async.t_cljs$core$async43517.prototype.cljs$core$async$Mix$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async43517.prototype.cljs$core$async$Mix$admix_STAR_$arity$2 = (function (_,ch){
var self__ = this;
var ___$1 = this;
cljs.core.swap_BANG_.call(null,self__.cs,cljs.core.assoc,ch,cljs.core.PersistentArrayMap.EMPTY);

return self__.changed.call(null);
}));

(cljs.core.async.t_cljs$core$async43517.prototype.cljs$core$async$Mix$unmix_STAR_$arity$2 = (function (_,ch){
var self__ = this;
var ___$1 = this;
cljs.core.swap_BANG_.call(null,self__.cs,cljs.core.dissoc,ch);

return self__.changed.call(null);
}));

(cljs.core.async.t_cljs$core$async43517.prototype.cljs$core$async$Mix$unmix_all_STAR_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
cljs.core.reset_BANG_.call(null,self__.cs,cljs.core.PersistentArrayMap.EMPTY);

return self__.changed.call(null);
}));

(cljs.core.async.t_cljs$core$async43517.prototype.cljs$core$async$Mix$toggle_STAR_$arity$2 = (function (_,state_map){
var self__ = this;
var ___$1 = this;
cljs.core.swap_BANG_.call(null,self__.cs,cljs.core.partial.call(null,cljs.core.merge_with,cljs.core.merge),state_map);

return self__.changed.call(null);
}));

(cljs.core.async.t_cljs$core$async43517.prototype.cljs$core$async$Mix$solo_mode_STAR_$arity$2 = (function (_,mode){
var self__ = this;
var ___$1 = this;
if(cljs.core.truth_(self__.solo_modes.call(null,mode))){
} else {
throw (new Error(["Assert failed: ",["mode must be one of: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(self__.solo_modes)].join(''),"\n","(solo-modes mode)"].join('')));
}

cljs.core.reset_BANG_.call(null,self__.solo_mode,mode);

return self__.changed.call(null);
}));

(cljs.core.async.t_cljs$core$async43517.getBasis = (function (){
return new cljs.core.PersistentVector(null, 10, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"change","change",477485025,null),new cljs.core.Symbol(null,"solo-mode","solo-mode",2031788074,null),new cljs.core.Symbol(null,"pick","pick",1300068175,null),new cljs.core.Symbol(null,"cs","cs",-117024463,null),new cljs.core.Symbol(null,"calc-state","calc-state",-349968968,null),new cljs.core.Symbol(null,"out","out",729986010,null),new cljs.core.Symbol(null,"changed","changed",-2083710852,null),new cljs.core.Symbol(null,"solo-modes","solo-modes",882180540,null),new cljs.core.Symbol(null,"attrs","attrs",-450137186,null),new cljs.core.Symbol(null,"meta43518","meta43518",-1234378772,null)], null);
}));

(cljs.core.async.t_cljs$core$async43517.cljs$lang$type = true);

(cljs.core.async.t_cljs$core$async43517.cljs$lang$ctorStr = "cljs.core.async/t_cljs$core$async43517");

(cljs.core.async.t_cljs$core$async43517.cljs$lang$ctorPrWriter = (function (this__4369__auto__,writer__4370__auto__,opt__4371__auto__){
return cljs.core._write.call(null,writer__4370__auto__,"cljs.core.async/t_cljs$core$async43517");
}));

/**
 * Positional factory function for cljs.core.async/t_cljs$core$async43517.
 */
cljs.core.async.__GT_t_cljs$core$async43517 = (function cljs$core$async$mix_$___GT_t_cljs$core$async43517(change__$1,solo_mode__$1,pick__$1,cs__$1,calc_state__$1,out__$1,changed__$1,solo_modes__$1,attrs__$1,meta43518){
return (new cljs.core.async.t_cljs$core$async43517(change__$1,solo_mode__$1,pick__$1,cs__$1,calc_state__$1,out__$1,changed__$1,solo_modes__$1,attrs__$1,meta43518));
});

}

return (new cljs.core.async.t_cljs$core$async43517(change,solo_mode,pick,cs,calc_state,out,changed,solo_modes,attrs,cljs.core.PersistentArrayMap.EMPTY));
})()
;
var c__26800__auto___43681 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_43621){
var state_val_43622 = (state_43621[(1)]);
if((state_val_43622 === (7))){
var inst_43536 = (state_43621[(2)]);
var state_43621__$1 = state_43621;
var statearr_43623_43682 = state_43621__$1;
(statearr_43623_43682[(2)] = inst_43536);

(statearr_43623_43682[(1)] = (4));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (20))){
var inst_43548 = (state_43621[(7)]);
var state_43621__$1 = state_43621;
var statearr_43624_43683 = state_43621__$1;
(statearr_43624_43683[(2)] = inst_43548);

(statearr_43624_43683[(1)] = (21));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (27))){
var state_43621__$1 = state_43621;
var statearr_43625_43684 = state_43621__$1;
(statearr_43625_43684[(2)] = null);

(statearr_43625_43684[(1)] = (28));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (1))){
var inst_43523 = (state_43621[(8)]);
var inst_43523__$1 = calc_state.call(null);
var inst_43525 = (inst_43523__$1 == null);
var inst_43526 = cljs.core.not.call(null,inst_43525);
var state_43621__$1 = (function (){var statearr_43626 = state_43621;
(statearr_43626[(8)] = inst_43523__$1);

return statearr_43626;
})();
if(inst_43526){
var statearr_43627_43685 = state_43621__$1;
(statearr_43627_43685[(1)] = (2));

} else {
var statearr_43628_43686 = state_43621__$1;
(statearr_43628_43686[(1)] = (3));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (24))){
var inst_43572 = (state_43621[(9)]);
var inst_43595 = (state_43621[(10)]);
var inst_43581 = (state_43621[(11)]);
var inst_43595__$1 = inst_43572.call(null,inst_43581);
var state_43621__$1 = (function (){var statearr_43629 = state_43621;
(statearr_43629[(10)] = inst_43595__$1);

return statearr_43629;
})();
if(cljs.core.truth_(inst_43595__$1)){
var statearr_43630_43687 = state_43621__$1;
(statearr_43630_43687[(1)] = (29));

} else {
var statearr_43631_43688 = state_43621__$1;
(statearr_43631_43688[(1)] = (30));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (4))){
var inst_43539 = (state_43621[(2)]);
var state_43621__$1 = state_43621;
if(cljs.core.truth_(inst_43539)){
var statearr_43632_43689 = state_43621__$1;
(statearr_43632_43689[(1)] = (8));

} else {
var statearr_43633_43690 = state_43621__$1;
(statearr_43633_43690[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (15))){
var inst_43566 = (state_43621[(2)]);
var state_43621__$1 = state_43621;
if(cljs.core.truth_(inst_43566)){
var statearr_43634_43691 = state_43621__$1;
(statearr_43634_43691[(1)] = (19));

} else {
var statearr_43635_43692 = state_43621__$1;
(statearr_43635_43692[(1)] = (20));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (21))){
var inst_43571 = (state_43621[(12)]);
var inst_43571__$1 = (state_43621[(2)]);
var inst_43572 = cljs.core.get.call(null,inst_43571__$1,new cljs.core.Keyword(null,"solos","solos",1441458643));
var inst_43573 = cljs.core.get.call(null,inst_43571__$1,new cljs.core.Keyword(null,"mutes","mutes",1068806309));
var inst_43574 = cljs.core.get.call(null,inst_43571__$1,new cljs.core.Keyword(null,"reads","reads",-1215067361));
var state_43621__$1 = (function (){var statearr_43636 = state_43621;
(statearr_43636[(9)] = inst_43572);

(statearr_43636[(13)] = inst_43573);

(statearr_43636[(12)] = inst_43571__$1);

return statearr_43636;
})();
return cljs.core.async.ioc_alts_BANG_.call(null,state_43621__$1,(22),inst_43574);
} else {
if((state_val_43622 === (31))){
var inst_43603 = (state_43621[(2)]);
var state_43621__$1 = state_43621;
if(cljs.core.truth_(inst_43603)){
var statearr_43637_43693 = state_43621__$1;
(statearr_43637_43693[(1)] = (32));

} else {
var statearr_43638_43694 = state_43621__$1;
(statearr_43638_43694[(1)] = (33));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (32))){
var inst_43580 = (state_43621[(14)]);
var state_43621__$1 = state_43621;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_43621__$1,(35),out,inst_43580);
} else {
if((state_val_43622 === (33))){
var inst_43571 = (state_43621[(12)]);
var inst_43548 = inst_43571;
var state_43621__$1 = (function (){var statearr_43639 = state_43621;
(statearr_43639[(7)] = inst_43548);

return statearr_43639;
})();
var statearr_43640_43695 = state_43621__$1;
(statearr_43640_43695[(2)] = null);

(statearr_43640_43695[(1)] = (11));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (13))){
var inst_43548 = (state_43621[(7)]);
var inst_43555 = inst_43548.cljs$lang$protocol_mask$partition0$;
var inst_43556 = (inst_43555 & (64));
var inst_43557 = inst_43548.cljs$core$ISeq$;
var inst_43558 = (cljs.core.PROTOCOL_SENTINEL === inst_43557);
var inst_43559 = ((inst_43556) || (inst_43558));
var state_43621__$1 = state_43621;
if(cljs.core.truth_(inst_43559)){
var statearr_43641_43696 = state_43621__$1;
(statearr_43641_43696[(1)] = (16));

} else {
var statearr_43642_43697 = state_43621__$1;
(statearr_43642_43697[(1)] = (17));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (22))){
var inst_43580 = (state_43621[(14)]);
var inst_43581 = (state_43621[(11)]);
var inst_43579 = (state_43621[(2)]);
var inst_43580__$1 = cljs.core.nth.call(null,inst_43579,(0),null);
var inst_43581__$1 = cljs.core.nth.call(null,inst_43579,(1),null);
var inst_43582 = (inst_43580__$1 == null);
var inst_43583 = cljs.core._EQ_.call(null,inst_43581__$1,change);
var inst_43584 = ((inst_43582) || (inst_43583));
var state_43621__$1 = (function (){var statearr_43643 = state_43621;
(statearr_43643[(14)] = inst_43580__$1);

(statearr_43643[(11)] = inst_43581__$1);

return statearr_43643;
})();
if(cljs.core.truth_(inst_43584)){
var statearr_43644_43698 = state_43621__$1;
(statearr_43644_43698[(1)] = (23));

} else {
var statearr_43645_43699 = state_43621__$1;
(statearr_43645_43699[(1)] = (24));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (36))){
var inst_43571 = (state_43621[(12)]);
var inst_43548 = inst_43571;
var state_43621__$1 = (function (){var statearr_43646 = state_43621;
(statearr_43646[(7)] = inst_43548);

return statearr_43646;
})();
var statearr_43647_43700 = state_43621__$1;
(statearr_43647_43700[(2)] = null);

(statearr_43647_43700[(1)] = (11));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (29))){
var inst_43595 = (state_43621[(10)]);
var state_43621__$1 = state_43621;
var statearr_43648_43701 = state_43621__$1;
(statearr_43648_43701[(2)] = inst_43595);

(statearr_43648_43701[(1)] = (31));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (6))){
var state_43621__$1 = state_43621;
var statearr_43649_43702 = state_43621__$1;
(statearr_43649_43702[(2)] = false);

(statearr_43649_43702[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (28))){
var inst_43591 = (state_43621[(2)]);
var inst_43592 = calc_state.call(null);
var inst_43548 = inst_43592;
var state_43621__$1 = (function (){var statearr_43650 = state_43621;
(statearr_43650[(7)] = inst_43548);

(statearr_43650[(15)] = inst_43591);

return statearr_43650;
})();
var statearr_43651_43703 = state_43621__$1;
(statearr_43651_43703[(2)] = null);

(statearr_43651_43703[(1)] = (11));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (25))){
var inst_43617 = (state_43621[(2)]);
var state_43621__$1 = state_43621;
var statearr_43652_43704 = state_43621__$1;
(statearr_43652_43704[(2)] = inst_43617);

(statearr_43652_43704[(1)] = (12));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (34))){
var inst_43615 = (state_43621[(2)]);
var state_43621__$1 = state_43621;
var statearr_43653_43705 = state_43621__$1;
(statearr_43653_43705[(2)] = inst_43615);

(statearr_43653_43705[(1)] = (25));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (17))){
var state_43621__$1 = state_43621;
var statearr_43654_43706 = state_43621__$1;
(statearr_43654_43706[(2)] = false);

(statearr_43654_43706[(1)] = (18));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (3))){
var state_43621__$1 = state_43621;
var statearr_43655_43707 = state_43621__$1;
(statearr_43655_43707[(2)] = false);

(statearr_43655_43707[(1)] = (4));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (12))){
var inst_43619 = (state_43621[(2)]);
var state_43621__$1 = state_43621;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_43621__$1,inst_43619);
} else {
if((state_val_43622 === (2))){
var inst_43523 = (state_43621[(8)]);
var inst_43528 = inst_43523.cljs$lang$protocol_mask$partition0$;
var inst_43529 = (inst_43528 & (64));
var inst_43530 = inst_43523.cljs$core$ISeq$;
var inst_43531 = (cljs.core.PROTOCOL_SENTINEL === inst_43530);
var inst_43532 = ((inst_43529) || (inst_43531));
var state_43621__$1 = state_43621;
if(cljs.core.truth_(inst_43532)){
var statearr_43656_43708 = state_43621__$1;
(statearr_43656_43708[(1)] = (5));

} else {
var statearr_43657_43709 = state_43621__$1;
(statearr_43657_43709[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (23))){
var inst_43580 = (state_43621[(14)]);
var inst_43586 = (inst_43580 == null);
var state_43621__$1 = state_43621;
if(cljs.core.truth_(inst_43586)){
var statearr_43658_43710 = state_43621__$1;
(statearr_43658_43710[(1)] = (26));

} else {
var statearr_43659_43711 = state_43621__$1;
(statearr_43659_43711[(1)] = (27));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (35))){
var inst_43606 = (state_43621[(2)]);
var state_43621__$1 = state_43621;
if(cljs.core.truth_(inst_43606)){
var statearr_43660_43712 = state_43621__$1;
(statearr_43660_43712[(1)] = (36));

} else {
var statearr_43661_43713 = state_43621__$1;
(statearr_43661_43713[(1)] = (37));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (19))){
var inst_43548 = (state_43621[(7)]);
var inst_43568 = cljs.core.apply.call(null,cljs.core.hash_map,inst_43548);
var state_43621__$1 = state_43621;
var statearr_43662_43714 = state_43621__$1;
(statearr_43662_43714[(2)] = inst_43568);

(statearr_43662_43714[(1)] = (21));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (11))){
var inst_43548 = (state_43621[(7)]);
var inst_43552 = (inst_43548 == null);
var inst_43553 = cljs.core.not.call(null,inst_43552);
var state_43621__$1 = state_43621;
if(inst_43553){
var statearr_43663_43715 = state_43621__$1;
(statearr_43663_43715[(1)] = (13));

} else {
var statearr_43664_43716 = state_43621__$1;
(statearr_43664_43716[(1)] = (14));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (9))){
var inst_43523 = (state_43621[(8)]);
var state_43621__$1 = state_43621;
var statearr_43665_43717 = state_43621__$1;
(statearr_43665_43717[(2)] = inst_43523);

(statearr_43665_43717[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (5))){
var state_43621__$1 = state_43621;
var statearr_43666_43718 = state_43621__$1;
(statearr_43666_43718[(2)] = true);

(statearr_43666_43718[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (14))){
var state_43621__$1 = state_43621;
var statearr_43667_43719 = state_43621__$1;
(statearr_43667_43719[(2)] = false);

(statearr_43667_43719[(1)] = (15));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (26))){
var inst_43581 = (state_43621[(11)]);
var inst_43588 = cljs.core.swap_BANG_.call(null,cs,cljs.core.dissoc,inst_43581);
var state_43621__$1 = state_43621;
var statearr_43668_43720 = state_43621__$1;
(statearr_43668_43720[(2)] = inst_43588);

(statearr_43668_43720[(1)] = (28));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (16))){
var state_43621__$1 = state_43621;
var statearr_43669_43721 = state_43621__$1;
(statearr_43669_43721[(2)] = true);

(statearr_43669_43721[(1)] = (18));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (38))){
var inst_43611 = (state_43621[(2)]);
var state_43621__$1 = state_43621;
var statearr_43670_43722 = state_43621__$1;
(statearr_43670_43722[(2)] = inst_43611);

(statearr_43670_43722[(1)] = (34));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (30))){
var inst_43572 = (state_43621[(9)]);
var inst_43573 = (state_43621[(13)]);
var inst_43581 = (state_43621[(11)]);
var inst_43598 = cljs.core.empty_QMARK_.call(null,inst_43572);
var inst_43599 = inst_43573.call(null,inst_43581);
var inst_43600 = cljs.core.not.call(null,inst_43599);
var inst_43601 = ((inst_43598) && (inst_43600));
var state_43621__$1 = state_43621;
var statearr_43671_43723 = state_43621__$1;
(statearr_43671_43723[(2)] = inst_43601);

(statearr_43671_43723[(1)] = (31));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (10))){
var inst_43523 = (state_43621[(8)]);
var inst_43544 = (state_43621[(2)]);
var inst_43545 = cljs.core.get.call(null,inst_43544,new cljs.core.Keyword(null,"solos","solos",1441458643));
var inst_43546 = cljs.core.get.call(null,inst_43544,new cljs.core.Keyword(null,"mutes","mutes",1068806309));
var inst_43547 = cljs.core.get.call(null,inst_43544,new cljs.core.Keyword(null,"reads","reads",-1215067361));
var inst_43548 = inst_43523;
var state_43621__$1 = (function (){var statearr_43672 = state_43621;
(statearr_43672[(7)] = inst_43548);

(statearr_43672[(16)] = inst_43546);

(statearr_43672[(17)] = inst_43547);

(statearr_43672[(18)] = inst_43545);

return statearr_43672;
})();
var statearr_43673_43724 = state_43621__$1;
(statearr_43673_43724[(2)] = null);

(statearr_43673_43724[(1)] = (11));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (18))){
var inst_43563 = (state_43621[(2)]);
var state_43621__$1 = state_43621;
var statearr_43674_43725 = state_43621__$1;
(statearr_43674_43725[(2)] = inst_43563);

(statearr_43674_43725[(1)] = (15));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (37))){
var state_43621__$1 = state_43621;
var statearr_43675_43726 = state_43621__$1;
(statearr_43675_43726[(2)] = null);

(statearr_43675_43726[(1)] = (38));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43622 === (8))){
var inst_43523 = (state_43621[(8)]);
var inst_43541 = cljs.core.apply.call(null,cljs.core.hash_map,inst_43523);
var state_43621__$1 = state_43621;
var statearr_43676_43727 = state_43621__$1;
(statearr_43676_43727[(2)] = inst_43541);

(statearr_43676_43727[(1)] = (10));


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
});
return (function() {
var cljs$core$async$mix_$_state_machine__26706__auto__ = null;
var cljs$core$async$mix_$_state_machine__26706__auto____0 = (function (){
var statearr_43677 = [null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_43677[(0)] = cljs$core$async$mix_$_state_machine__26706__auto__);

(statearr_43677[(1)] = (1));

return statearr_43677;
});
var cljs$core$async$mix_$_state_machine__26706__auto____1 = (function (state_43621){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_43621);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e43678){if((e43678 instanceof Object)){
var ex__26709__auto__ = e43678;
var statearr_43679_43728 = state_43621;
(statearr_43679_43728[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_43621);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e43678;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__43729 = state_43621;
state_43621 = G__43729;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$mix_$_state_machine__26706__auto__ = function(state_43621){
switch(arguments.length){
case 0:
return cljs$core$async$mix_$_state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$mix_$_state_machine__26706__auto____1.call(this,state_43621);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$mix_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$mix_$_state_machine__26706__auto____0;
cljs$core$async$mix_$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$mix_$_state_machine__26706__auto____1;
return cljs$core$async$mix_$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_43680 = f__26801__auto__.call(null);
(statearr_43680[(6)] = c__26800__auto___43681);

return statearr_43680;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return m;
});
/**
 * Adds ch as an input to the mix
 */
cljs.core.async.admix = (function cljs$core$async$admix(mix,ch){
return cljs.core.async.admix_STAR_.call(null,mix,ch);
});
/**
 * Removes ch as an input to the mix
 */
cljs.core.async.unmix = (function cljs$core$async$unmix(mix,ch){
return cljs.core.async.unmix_STAR_.call(null,mix,ch);
});
/**
 * removes all inputs from the mix
 */
cljs.core.async.unmix_all = (function cljs$core$async$unmix_all(mix){
return cljs.core.async.unmix_all_STAR_.call(null,mix);
});
/**
 * Atomically sets the state(s) of one or more channels in a mix. The
 *   state map is a map of channels -> channel-state-map. A
 *   channel-state-map is a map of attrs -> boolean, where attr is one or
 *   more of :mute, :pause or :solo. Any states supplied are merged with
 *   the current state.
 * 
 *   Note that channels can be added to a mix via toggle, which can be
 *   used to add channels in a particular (e.g. paused) state.
 */
cljs.core.async.toggle = (function cljs$core$async$toggle(mix,state_map){
return cljs.core.async.toggle_STAR_.call(null,mix,state_map);
});
/**
 * Sets the solo mode of the mix. mode must be one of :mute or :pause
 */
cljs.core.async.solo_mode = (function cljs$core$async$solo_mode(mix,mode){
return cljs.core.async.solo_mode_STAR_.call(null,mix,mode);
});

/**
 * @interface
 */
cljs.core.async.Pub = function(){};

var cljs$core$async$Pub$sub_STAR_$dyn_43732 = (function (p,v,ch,close_QMARK_){
var x__4428__auto__ = (((p == null))?null:p);
var m__4429__auto__ = (cljs.core.async.sub_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,p,v,ch,close_QMARK_);
} else {
var m__4426__auto__ = (cljs.core.async.sub_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,p,v,ch,close_QMARK_);
} else {
throw cljs.core.missing_protocol.call(null,"Pub.sub*",p);
}
}
});
cljs.core.async.sub_STAR_ = (function cljs$core$async$sub_STAR_(p,v,ch,close_QMARK_){
if((((!((p == null)))) && ((!((p.cljs$core$async$Pub$sub_STAR_$arity$4 == null)))))){
return p.cljs$core$async$Pub$sub_STAR_$arity$4(p,v,ch,close_QMARK_);
} else {
return cljs$core$async$Pub$sub_STAR_$dyn_43732.call(null,p,v,ch,close_QMARK_);
}
});

var cljs$core$async$Pub$unsub_STAR_$dyn_43733 = (function (p,v,ch){
var x__4428__auto__ = (((p == null))?null:p);
var m__4429__auto__ = (cljs.core.async.unsub_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,p,v,ch);
} else {
var m__4426__auto__ = (cljs.core.async.unsub_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,p,v,ch);
} else {
throw cljs.core.missing_protocol.call(null,"Pub.unsub*",p);
}
}
});
cljs.core.async.unsub_STAR_ = (function cljs$core$async$unsub_STAR_(p,v,ch){
if((((!((p == null)))) && ((!((p.cljs$core$async$Pub$unsub_STAR_$arity$3 == null)))))){
return p.cljs$core$async$Pub$unsub_STAR_$arity$3(p,v,ch);
} else {
return cljs$core$async$Pub$unsub_STAR_$dyn_43733.call(null,p,v,ch);
}
});

var cljs$core$async$Pub$unsub_all_STAR_$dyn_43734 = (function() {
var G__43735 = null;
var G__43735__1 = (function (p){
var x__4428__auto__ = (((p == null))?null:p);
var m__4429__auto__ = (cljs.core.async.unsub_all_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,p);
} else {
var m__4426__auto__ = (cljs.core.async.unsub_all_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,p);
} else {
throw cljs.core.missing_protocol.call(null,"Pub.unsub-all*",p);
}
}
});
var G__43735__2 = (function (p,v){
var x__4428__auto__ = (((p == null))?null:p);
var m__4429__auto__ = (cljs.core.async.unsub_all_STAR_[goog.typeOf(x__4428__auto__)]);
if((!((m__4429__auto__ == null)))){
return m__4429__auto__.call(null,p,v);
} else {
var m__4426__auto__ = (cljs.core.async.unsub_all_STAR_["_"]);
if((!((m__4426__auto__ == null)))){
return m__4426__auto__.call(null,p,v);
} else {
throw cljs.core.missing_protocol.call(null,"Pub.unsub-all*",p);
}
}
});
G__43735 = function(p,v){
switch(arguments.length){
case 1:
return G__43735__1.call(this,p);
case 2:
return G__43735__2.call(this,p,v);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
G__43735.cljs$core$IFn$_invoke$arity$1 = G__43735__1;
G__43735.cljs$core$IFn$_invoke$arity$2 = G__43735__2;
return G__43735;
})()
;
cljs.core.async.unsub_all_STAR_ = (function cljs$core$async$unsub_all_STAR_(var_args){
var G__43731 = arguments.length;
switch (G__43731) {
case 1:
return cljs.core.async.unsub_all_STAR_.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 2:
return cljs.core.async.unsub_all_STAR_.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.unsub_all_STAR_.cljs$core$IFn$_invoke$arity$1 = (function (p){
if((((!((p == null)))) && ((!((p.cljs$core$async$Pub$unsub_all_STAR_$arity$1 == null)))))){
return p.cljs$core$async$Pub$unsub_all_STAR_$arity$1(p);
} else {
return cljs$core$async$Pub$unsub_all_STAR_$dyn_43734.call(null,p);
}
}));

(cljs.core.async.unsub_all_STAR_.cljs$core$IFn$_invoke$arity$2 = (function (p,v){
if((((!((p == null)))) && ((!((p.cljs$core$async$Pub$unsub_all_STAR_$arity$2 == null)))))){
return p.cljs$core$async$Pub$unsub_all_STAR_$arity$2(p,v);
} else {
return cljs$core$async$Pub$unsub_all_STAR_$dyn_43734.call(null,p,v);
}
}));

(cljs.core.async.unsub_all_STAR_.cljs$lang$maxFixedArity = 2);


/**
 * Creates and returns a pub(lication) of the supplied channel,
 *   partitioned into topics by the topic-fn. topic-fn will be applied to
 *   each value on the channel and the result will determine the 'topic'
 *   on which that value will be put. Channels can be subscribed to
 *   receive copies of topics using 'sub', and unsubscribed using
 *   'unsub'. Each topic will be handled by an internal mult on a
 *   dedicated channel. By default these internal channels are
 *   unbuffered, but a buf-fn can be supplied which, given a topic,
 *   creates a buffer with desired properties.
 * 
 *   Each item is distributed to all subs in parallel and synchronously,
 *   i.e. each sub must accept before the next item is distributed. Use
 *   buffering/windowing to prevent slow subs from holding up the pub.
 * 
 *   Items received when there are no matching subs get dropped.
 * 
 *   Note that if buf-fns are used then each topic is handled
 *   asynchronously, i.e. if a channel is subscribed to more than one
 *   topic it should not expect them to be interleaved identically with
 *   the source.
 */
cljs.core.async.pub = (function cljs$core$async$pub(var_args){
var G__43739 = arguments.length;
switch (G__43739) {
case 2:
return cljs.core.async.pub.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.pub.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.pub.cljs$core$IFn$_invoke$arity$2 = (function (ch,topic_fn){
return cljs.core.async.pub.call(null,ch,topic_fn,cljs.core.constantly.call(null,null));
}));

(cljs.core.async.pub.cljs$core$IFn$_invoke$arity$3 = (function (ch,topic_fn,buf_fn){
var mults = cljs.core.atom.call(null,cljs.core.PersistentArrayMap.EMPTY);
var ensure_mult = (function (topic){
var or__4126__auto__ = cljs.core.get.call(null,cljs.core.deref.call(null,mults),topic);
if(cljs.core.truth_(or__4126__auto__)){
return or__4126__auto__;
} else {
return cljs.core.get.call(null,cljs.core.swap_BANG_.call(null,mults,(function (p1__43737_SHARP_){
if(cljs.core.truth_(p1__43737_SHARP_.call(null,topic))){
return p1__43737_SHARP_;
} else {
return cljs.core.assoc.call(null,p1__43737_SHARP_,topic,cljs.core.async.mult.call(null,cljs.core.async.chan.call(null,buf_fn.call(null,topic))));
}
})),topic);
}
});
var p = (function (){
if((typeof cljs !== 'undefined') && (typeof cljs.core !== 'undefined') && (typeof cljs.core.async !== 'undefined') && (typeof cljs.core.async.t_cljs$core$async43740 !== 'undefined')){
} else {

/**
* @constructor
 * @implements {cljs.core.async.Pub}
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.async.Mux}
 * @implements {cljs.core.IWithMeta}
*/
cljs.core.async.t_cljs$core$async43740 = (function (ch,topic_fn,buf_fn,mults,ensure_mult,meta43741){
this.ch = ch;
this.topic_fn = topic_fn;
this.buf_fn = buf_fn;
this.mults = mults;
this.ensure_mult = ensure_mult;
this.meta43741 = meta43741;
this.cljs$lang$protocol_mask$partition0$ = 393216;
this.cljs$lang$protocol_mask$partition1$ = 0;
});
(cljs.core.async.t_cljs$core$async43740.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (_43742,meta43741__$1){
var self__ = this;
var _43742__$1 = this;
return (new cljs.core.async.t_cljs$core$async43740(self__.ch,self__.topic_fn,self__.buf_fn,self__.mults,self__.ensure_mult,meta43741__$1));
}));

(cljs.core.async.t_cljs$core$async43740.prototype.cljs$core$IMeta$_meta$arity$1 = (function (_43742){
var self__ = this;
var _43742__$1 = this;
return self__.meta43741;
}));

(cljs.core.async.t_cljs$core$async43740.prototype.cljs$core$async$Mux$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async43740.prototype.cljs$core$async$Mux$muxch_STAR_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return self__.ch;
}));

(cljs.core.async.t_cljs$core$async43740.prototype.cljs$core$async$Pub$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async43740.prototype.cljs$core$async$Pub$sub_STAR_$arity$4 = (function (p,topic,ch__$1,close_QMARK_){
var self__ = this;
var p__$1 = this;
var m = self__.ensure_mult.call(null,topic);
return cljs.core.async.tap.call(null,m,ch__$1,close_QMARK_);
}));

(cljs.core.async.t_cljs$core$async43740.prototype.cljs$core$async$Pub$unsub_STAR_$arity$3 = (function (p,topic,ch__$1){
var self__ = this;
var p__$1 = this;
var temp__5720__auto__ = cljs.core.get.call(null,cljs.core.deref.call(null,self__.mults),topic);
if(cljs.core.truth_(temp__5720__auto__)){
var m = temp__5720__auto__;
return cljs.core.async.untap.call(null,m,ch__$1);
} else {
return null;
}
}));

(cljs.core.async.t_cljs$core$async43740.prototype.cljs$core$async$Pub$unsub_all_STAR_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return cljs.core.reset_BANG_.call(null,self__.mults,cljs.core.PersistentArrayMap.EMPTY);
}));

(cljs.core.async.t_cljs$core$async43740.prototype.cljs$core$async$Pub$unsub_all_STAR_$arity$2 = (function (_,topic){
var self__ = this;
var ___$1 = this;
return cljs.core.swap_BANG_.call(null,self__.mults,cljs.core.dissoc,topic);
}));

(cljs.core.async.t_cljs$core$async43740.getBasis = (function (){
return new cljs.core.PersistentVector(null, 6, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"ch","ch",1085813622,null),new cljs.core.Symbol(null,"topic-fn","topic-fn",-862449736,null),new cljs.core.Symbol(null,"buf-fn","buf-fn",-1200281591,null),new cljs.core.Symbol(null,"mults","mults",-461114485,null),new cljs.core.Symbol(null,"ensure-mult","ensure-mult",1796584816,null),new cljs.core.Symbol(null,"meta43741","meta43741",-725169338,null)], null);
}));

(cljs.core.async.t_cljs$core$async43740.cljs$lang$type = true);

(cljs.core.async.t_cljs$core$async43740.cljs$lang$ctorStr = "cljs.core.async/t_cljs$core$async43740");

(cljs.core.async.t_cljs$core$async43740.cljs$lang$ctorPrWriter = (function (this__4369__auto__,writer__4370__auto__,opt__4371__auto__){
return cljs.core._write.call(null,writer__4370__auto__,"cljs.core.async/t_cljs$core$async43740");
}));

/**
 * Positional factory function for cljs.core.async/t_cljs$core$async43740.
 */
cljs.core.async.__GT_t_cljs$core$async43740 = (function cljs$core$async$__GT_t_cljs$core$async43740(ch__$1,topic_fn__$1,buf_fn__$1,mults__$1,ensure_mult__$1,meta43741){
return (new cljs.core.async.t_cljs$core$async43740(ch__$1,topic_fn__$1,buf_fn__$1,mults__$1,ensure_mult__$1,meta43741));
});

}

return (new cljs.core.async.t_cljs$core$async43740(ch,topic_fn,buf_fn,mults,ensure_mult,cljs.core.PersistentArrayMap.EMPTY));
})()
;
var c__26800__auto___43860 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_43814){
var state_val_43815 = (state_43814[(1)]);
if((state_val_43815 === (7))){
var inst_43810 = (state_43814[(2)]);
var state_43814__$1 = state_43814;
var statearr_43816_43861 = state_43814__$1;
(statearr_43816_43861[(2)] = inst_43810);

(statearr_43816_43861[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (20))){
var state_43814__$1 = state_43814;
var statearr_43817_43862 = state_43814__$1;
(statearr_43817_43862[(2)] = null);

(statearr_43817_43862[(1)] = (21));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (1))){
var state_43814__$1 = state_43814;
var statearr_43818_43863 = state_43814__$1;
(statearr_43818_43863[(2)] = null);

(statearr_43818_43863[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (24))){
var inst_43793 = (state_43814[(7)]);
var inst_43802 = cljs.core.swap_BANG_.call(null,mults,cljs.core.dissoc,inst_43793);
var state_43814__$1 = state_43814;
var statearr_43819_43864 = state_43814__$1;
(statearr_43819_43864[(2)] = inst_43802);

(statearr_43819_43864[(1)] = (25));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (4))){
var inst_43745 = (state_43814[(8)]);
var inst_43745__$1 = (state_43814[(2)]);
var inst_43746 = (inst_43745__$1 == null);
var state_43814__$1 = (function (){var statearr_43820 = state_43814;
(statearr_43820[(8)] = inst_43745__$1);

return statearr_43820;
})();
if(cljs.core.truth_(inst_43746)){
var statearr_43821_43865 = state_43814__$1;
(statearr_43821_43865[(1)] = (5));

} else {
var statearr_43822_43866 = state_43814__$1;
(statearr_43822_43866[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (15))){
var inst_43787 = (state_43814[(2)]);
var state_43814__$1 = state_43814;
var statearr_43823_43867 = state_43814__$1;
(statearr_43823_43867[(2)] = inst_43787);

(statearr_43823_43867[(1)] = (12));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (21))){
var inst_43807 = (state_43814[(2)]);
var state_43814__$1 = (function (){var statearr_43824 = state_43814;
(statearr_43824[(9)] = inst_43807);

return statearr_43824;
})();
var statearr_43825_43868 = state_43814__$1;
(statearr_43825_43868[(2)] = null);

(statearr_43825_43868[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (13))){
var inst_43769 = (state_43814[(10)]);
var inst_43771 = cljs.core.chunked_seq_QMARK_.call(null,inst_43769);
var state_43814__$1 = state_43814;
if(inst_43771){
var statearr_43826_43869 = state_43814__$1;
(statearr_43826_43869[(1)] = (16));

} else {
var statearr_43827_43870 = state_43814__$1;
(statearr_43827_43870[(1)] = (17));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (22))){
var inst_43799 = (state_43814[(2)]);
var state_43814__$1 = state_43814;
if(cljs.core.truth_(inst_43799)){
var statearr_43828_43871 = state_43814__$1;
(statearr_43828_43871[(1)] = (23));

} else {
var statearr_43829_43872 = state_43814__$1;
(statearr_43829_43872[(1)] = (24));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (6))){
var inst_43795 = (state_43814[(11)]);
var inst_43793 = (state_43814[(7)]);
var inst_43745 = (state_43814[(8)]);
var inst_43793__$1 = topic_fn.call(null,inst_43745);
var inst_43794 = cljs.core.deref.call(null,mults);
var inst_43795__$1 = cljs.core.get.call(null,inst_43794,inst_43793__$1);
var state_43814__$1 = (function (){var statearr_43830 = state_43814;
(statearr_43830[(11)] = inst_43795__$1);

(statearr_43830[(7)] = inst_43793__$1);

return statearr_43830;
})();
if(cljs.core.truth_(inst_43795__$1)){
var statearr_43831_43873 = state_43814__$1;
(statearr_43831_43873[(1)] = (19));

} else {
var statearr_43832_43874 = state_43814__$1;
(statearr_43832_43874[(1)] = (20));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (25))){
var inst_43804 = (state_43814[(2)]);
var state_43814__$1 = state_43814;
var statearr_43833_43875 = state_43814__$1;
(statearr_43833_43875[(2)] = inst_43804);

(statearr_43833_43875[(1)] = (21));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (17))){
var inst_43769 = (state_43814[(10)]);
var inst_43778 = cljs.core.first.call(null,inst_43769);
var inst_43779 = cljs.core.async.muxch_STAR_.call(null,inst_43778);
var inst_43780 = cljs.core.async.close_BANG_.call(null,inst_43779);
var inst_43781 = cljs.core.next.call(null,inst_43769);
var inst_43755 = inst_43781;
var inst_43756 = null;
var inst_43757 = (0);
var inst_43758 = (0);
var state_43814__$1 = (function (){var statearr_43834 = state_43814;
(statearr_43834[(12)] = inst_43780);

(statearr_43834[(13)] = inst_43757);

(statearr_43834[(14)] = inst_43756);

(statearr_43834[(15)] = inst_43755);

(statearr_43834[(16)] = inst_43758);

return statearr_43834;
})();
var statearr_43835_43876 = state_43814__$1;
(statearr_43835_43876[(2)] = null);

(statearr_43835_43876[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (3))){
var inst_43812 = (state_43814[(2)]);
var state_43814__$1 = state_43814;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_43814__$1,inst_43812);
} else {
if((state_val_43815 === (12))){
var inst_43789 = (state_43814[(2)]);
var state_43814__$1 = state_43814;
var statearr_43836_43877 = state_43814__$1;
(statearr_43836_43877[(2)] = inst_43789);

(statearr_43836_43877[(1)] = (9));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (2))){
var state_43814__$1 = state_43814;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_43814__$1,(4),ch);
} else {
if((state_val_43815 === (23))){
var state_43814__$1 = state_43814;
var statearr_43837_43878 = state_43814__$1;
(statearr_43837_43878[(2)] = null);

(statearr_43837_43878[(1)] = (25));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (19))){
var inst_43795 = (state_43814[(11)]);
var inst_43745 = (state_43814[(8)]);
var inst_43797 = cljs.core.async.muxch_STAR_.call(null,inst_43795);
var state_43814__$1 = state_43814;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_43814__$1,(22),inst_43797,inst_43745);
} else {
if((state_val_43815 === (11))){
var inst_43755 = (state_43814[(15)]);
var inst_43769 = (state_43814[(10)]);
var inst_43769__$1 = cljs.core.seq.call(null,inst_43755);
var state_43814__$1 = (function (){var statearr_43838 = state_43814;
(statearr_43838[(10)] = inst_43769__$1);

return statearr_43838;
})();
if(inst_43769__$1){
var statearr_43839_43879 = state_43814__$1;
(statearr_43839_43879[(1)] = (13));

} else {
var statearr_43840_43880 = state_43814__$1;
(statearr_43840_43880[(1)] = (14));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (9))){
var inst_43791 = (state_43814[(2)]);
var state_43814__$1 = state_43814;
var statearr_43841_43881 = state_43814__$1;
(statearr_43841_43881[(2)] = inst_43791);

(statearr_43841_43881[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (5))){
var inst_43752 = cljs.core.deref.call(null,mults);
var inst_43753 = cljs.core.vals.call(null,inst_43752);
var inst_43754 = cljs.core.seq.call(null,inst_43753);
var inst_43755 = inst_43754;
var inst_43756 = null;
var inst_43757 = (0);
var inst_43758 = (0);
var state_43814__$1 = (function (){var statearr_43842 = state_43814;
(statearr_43842[(13)] = inst_43757);

(statearr_43842[(14)] = inst_43756);

(statearr_43842[(15)] = inst_43755);

(statearr_43842[(16)] = inst_43758);

return statearr_43842;
})();
var statearr_43843_43882 = state_43814__$1;
(statearr_43843_43882[(2)] = null);

(statearr_43843_43882[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (14))){
var state_43814__$1 = state_43814;
var statearr_43847_43883 = state_43814__$1;
(statearr_43847_43883[(2)] = null);

(statearr_43847_43883[(1)] = (15));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (16))){
var inst_43769 = (state_43814[(10)]);
var inst_43773 = cljs.core.chunk_first.call(null,inst_43769);
var inst_43774 = cljs.core.chunk_rest.call(null,inst_43769);
var inst_43775 = cljs.core.count.call(null,inst_43773);
var inst_43755 = inst_43774;
var inst_43756 = inst_43773;
var inst_43757 = inst_43775;
var inst_43758 = (0);
var state_43814__$1 = (function (){var statearr_43848 = state_43814;
(statearr_43848[(13)] = inst_43757);

(statearr_43848[(14)] = inst_43756);

(statearr_43848[(15)] = inst_43755);

(statearr_43848[(16)] = inst_43758);

return statearr_43848;
})();
var statearr_43849_43884 = state_43814__$1;
(statearr_43849_43884[(2)] = null);

(statearr_43849_43884[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (10))){
var inst_43757 = (state_43814[(13)]);
var inst_43756 = (state_43814[(14)]);
var inst_43755 = (state_43814[(15)]);
var inst_43758 = (state_43814[(16)]);
var inst_43763 = cljs.core._nth.call(null,inst_43756,inst_43758);
var inst_43764 = cljs.core.async.muxch_STAR_.call(null,inst_43763);
var inst_43765 = cljs.core.async.close_BANG_.call(null,inst_43764);
var inst_43766 = (inst_43758 + (1));
var tmp43844 = inst_43757;
var tmp43845 = inst_43756;
var tmp43846 = inst_43755;
var inst_43755__$1 = tmp43846;
var inst_43756__$1 = tmp43845;
var inst_43757__$1 = tmp43844;
var inst_43758__$1 = inst_43766;
var state_43814__$1 = (function (){var statearr_43850 = state_43814;
(statearr_43850[(17)] = inst_43765);

(statearr_43850[(13)] = inst_43757__$1);

(statearr_43850[(14)] = inst_43756__$1);

(statearr_43850[(15)] = inst_43755__$1);

(statearr_43850[(16)] = inst_43758__$1);

return statearr_43850;
})();
var statearr_43851_43885 = state_43814__$1;
(statearr_43851_43885[(2)] = null);

(statearr_43851_43885[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (18))){
var inst_43784 = (state_43814[(2)]);
var state_43814__$1 = state_43814;
var statearr_43852_43886 = state_43814__$1;
(statearr_43852_43886[(2)] = inst_43784);

(statearr_43852_43886[(1)] = (15));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43815 === (8))){
var inst_43757 = (state_43814[(13)]);
var inst_43758 = (state_43814[(16)]);
var inst_43760 = (inst_43758 < inst_43757);
var inst_43761 = inst_43760;
var state_43814__$1 = state_43814;
if(cljs.core.truth_(inst_43761)){
var statearr_43853_43887 = state_43814__$1;
(statearr_43853_43887[(1)] = (10));

} else {
var statearr_43854_43888 = state_43814__$1;
(statearr_43854_43888[(1)] = (11));

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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_43855 = [null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_43855[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_43855[(1)] = (1));

return statearr_43855;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_43814){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_43814);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e43856){if((e43856 instanceof Object)){
var ex__26709__auto__ = e43856;
var statearr_43857_43889 = state_43814;
(statearr_43857_43889[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_43814);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e43856;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__43890 = state_43814;
state_43814 = G__43890;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_43814){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_43814);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_43858 = f__26801__auto__.call(null);
(statearr_43858[(6)] = c__26800__auto___43860);

return statearr_43858;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return p;
}));

(cljs.core.async.pub.cljs$lang$maxFixedArity = 3);

/**
 * Subscribes a channel to a topic of a pub.
 * 
 *   By default the channel will be closed when the source closes,
 *   but can be determined by the close? parameter.
 */
cljs.core.async.sub = (function cljs$core$async$sub(var_args){
var G__43892 = arguments.length;
switch (G__43892) {
case 3:
return cljs.core.async.sub.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
case 4:
return cljs.core.async.sub.cljs$core$IFn$_invoke$arity$4((arguments[(0)]),(arguments[(1)]),(arguments[(2)]),(arguments[(3)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.sub.cljs$core$IFn$_invoke$arity$3 = (function (p,topic,ch){
return cljs.core.async.sub.call(null,p,topic,ch,true);
}));

(cljs.core.async.sub.cljs$core$IFn$_invoke$arity$4 = (function (p,topic,ch,close_QMARK_){
return cljs.core.async.sub_STAR_.call(null,p,topic,ch,close_QMARK_);
}));

(cljs.core.async.sub.cljs$lang$maxFixedArity = 4);

/**
 * Unsubscribes a channel from a topic of a pub
 */
cljs.core.async.unsub = (function cljs$core$async$unsub(p,topic,ch){
return cljs.core.async.unsub_STAR_.call(null,p,topic,ch);
});
/**
 * Unsubscribes all channels from a pub, or a topic of a pub
 */
cljs.core.async.unsub_all = (function cljs$core$async$unsub_all(var_args){
var G__43895 = arguments.length;
switch (G__43895) {
case 1:
return cljs.core.async.unsub_all.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 2:
return cljs.core.async.unsub_all.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.unsub_all.cljs$core$IFn$_invoke$arity$1 = (function (p){
return cljs.core.async.unsub_all_STAR_.call(null,p);
}));

(cljs.core.async.unsub_all.cljs$core$IFn$_invoke$arity$2 = (function (p,topic){
return cljs.core.async.unsub_all_STAR_.call(null,p,topic);
}));

(cljs.core.async.unsub_all.cljs$lang$maxFixedArity = 2);

/**
 * Takes a function and a collection of source channels, and returns a
 *   channel which contains the values produced by applying f to the set
 *   of first items taken from each source channel, followed by applying
 *   f to the set of second items from each channel, until any one of the
 *   channels is closed, at which point the output channel will be
 *   closed. The returned channel will be unbuffered by default, or a
 *   buf-or-n can be supplied
 */
cljs.core.async.map = (function cljs$core$async$map(var_args){
var G__43898 = arguments.length;
switch (G__43898) {
case 2:
return cljs.core.async.map.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.map.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.map.cljs$core$IFn$_invoke$arity$2 = (function (f,chs){
return cljs.core.async.map.call(null,f,chs,null);
}));

(cljs.core.async.map.cljs$core$IFn$_invoke$arity$3 = (function (f,chs,buf_or_n){
var chs__$1 = cljs.core.vec.call(null,chs);
var out = cljs.core.async.chan.call(null,buf_or_n);
var cnt = cljs.core.count.call(null,chs__$1);
var rets = cljs.core.object_array.call(null,cnt);
var dchan = cljs.core.async.chan.call(null,(1));
var dctr = cljs.core.atom.call(null,null);
var done = cljs.core.mapv.call(null,(function (i){
return (function (ret){
(rets[i] = ret);

if((cljs.core.swap_BANG_.call(null,dctr,cljs.core.dec) === (0))){
return cljs.core.async.put_BANG_.call(null,dchan,rets.slice((0)));
} else {
return null;
}
});
}),cljs.core.range.call(null,cnt));
var c__26800__auto___43965 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_43937){
var state_val_43938 = (state_43937[(1)]);
if((state_val_43938 === (7))){
var state_43937__$1 = state_43937;
var statearr_43939_43966 = state_43937__$1;
(statearr_43939_43966[(2)] = null);

(statearr_43939_43966[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (1))){
var state_43937__$1 = state_43937;
var statearr_43940_43967 = state_43937__$1;
(statearr_43940_43967[(2)] = null);

(statearr_43940_43967[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (4))){
var inst_43901 = (state_43937[(7)]);
var inst_43903 = (inst_43901 < cnt);
var state_43937__$1 = state_43937;
if(cljs.core.truth_(inst_43903)){
var statearr_43941_43968 = state_43937__$1;
(statearr_43941_43968[(1)] = (6));

} else {
var statearr_43942_43969 = state_43937__$1;
(statearr_43942_43969[(1)] = (7));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (15))){
var inst_43933 = (state_43937[(2)]);
var state_43937__$1 = state_43937;
var statearr_43943_43970 = state_43937__$1;
(statearr_43943_43970[(2)] = inst_43933);

(statearr_43943_43970[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (13))){
var inst_43926 = cljs.core.async.close_BANG_.call(null,out);
var state_43937__$1 = state_43937;
var statearr_43944_43971 = state_43937__$1;
(statearr_43944_43971[(2)] = inst_43926);

(statearr_43944_43971[(1)] = (15));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (6))){
var state_43937__$1 = state_43937;
var statearr_43945_43972 = state_43937__$1;
(statearr_43945_43972[(2)] = null);

(statearr_43945_43972[(1)] = (11));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (3))){
var inst_43935 = (state_43937[(2)]);
var state_43937__$1 = state_43937;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_43937__$1,inst_43935);
} else {
if((state_val_43938 === (12))){
var inst_43923 = (state_43937[(8)]);
var inst_43923__$1 = (state_43937[(2)]);
var inst_43924 = cljs.core.some.call(null,cljs.core.nil_QMARK_,inst_43923__$1);
var state_43937__$1 = (function (){var statearr_43946 = state_43937;
(statearr_43946[(8)] = inst_43923__$1);

return statearr_43946;
})();
if(cljs.core.truth_(inst_43924)){
var statearr_43947_43973 = state_43937__$1;
(statearr_43947_43973[(1)] = (13));

} else {
var statearr_43948_43974 = state_43937__$1;
(statearr_43948_43974[(1)] = (14));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (2))){
var inst_43900 = cljs.core.reset_BANG_.call(null,dctr,cnt);
var inst_43901 = (0);
var state_43937__$1 = (function (){var statearr_43949 = state_43937;
(statearr_43949[(7)] = inst_43901);

(statearr_43949[(9)] = inst_43900);

return statearr_43949;
})();
var statearr_43950_43975 = state_43937__$1;
(statearr_43950_43975[(2)] = null);

(statearr_43950_43975[(1)] = (4));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (11))){
var inst_43901 = (state_43937[(7)]);
var _ = cljs.core.async.impl.ioc_helpers.add_exception_frame.call(null,state_43937,(10),Object,null,(9));
var inst_43910 = chs__$1.call(null,inst_43901);
var inst_43911 = done.call(null,inst_43901);
var inst_43912 = cljs.core.async.take_BANG_.call(null,inst_43910,inst_43911);
var state_43937__$1 = state_43937;
var statearr_43951_43976 = state_43937__$1;
(statearr_43951_43976[(2)] = inst_43912);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_43937__$1);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (9))){
var inst_43901 = (state_43937[(7)]);
var inst_43914 = (state_43937[(2)]);
var inst_43915 = (inst_43901 + (1));
var inst_43901__$1 = inst_43915;
var state_43937__$1 = (function (){var statearr_43952 = state_43937;
(statearr_43952[(10)] = inst_43914);

(statearr_43952[(7)] = inst_43901__$1);

return statearr_43952;
})();
var statearr_43953_43977 = state_43937__$1;
(statearr_43953_43977[(2)] = null);

(statearr_43953_43977[(1)] = (4));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (5))){
var inst_43921 = (state_43937[(2)]);
var state_43937__$1 = (function (){var statearr_43954 = state_43937;
(statearr_43954[(11)] = inst_43921);

return statearr_43954;
})();
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_43937__$1,(12),dchan);
} else {
if((state_val_43938 === (14))){
var inst_43923 = (state_43937[(8)]);
var inst_43928 = cljs.core.apply.call(null,f,inst_43923);
var state_43937__$1 = state_43937;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_43937__$1,(16),out,inst_43928);
} else {
if((state_val_43938 === (16))){
var inst_43930 = (state_43937[(2)]);
var state_43937__$1 = (function (){var statearr_43955 = state_43937;
(statearr_43955[(12)] = inst_43930);

return statearr_43955;
})();
var statearr_43956_43978 = state_43937__$1;
(statearr_43956_43978[(2)] = null);

(statearr_43956_43978[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (10))){
var inst_43905 = (state_43937[(2)]);
var inst_43906 = cljs.core.swap_BANG_.call(null,dctr,cljs.core.dec);
var state_43937__$1 = (function (){var statearr_43957 = state_43937;
(statearr_43957[(13)] = inst_43905);

return statearr_43957;
})();
var statearr_43958_43979 = state_43937__$1;
(statearr_43958_43979[(2)] = inst_43906);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_43937__$1);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_43938 === (8))){
var inst_43919 = (state_43937[(2)]);
var state_43937__$1 = state_43937;
var statearr_43959_43980 = state_43937__$1;
(statearr_43959_43980[(2)] = inst_43919);

(statearr_43959_43980[(1)] = (5));


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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_43960 = [null,null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_43960[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_43960[(1)] = (1));

return statearr_43960;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_43937){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_43937);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e43961){if((e43961 instanceof Object)){
var ex__26709__auto__ = e43961;
var statearr_43962_43981 = state_43937;
(statearr_43962_43981[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_43937);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e43961;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__43982 = state_43937;
state_43937 = G__43982;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_43937){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_43937);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_43963 = f__26801__auto__.call(null);
(statearr_43963[(6)] = c__26800__auto___43965);

return statearr_43963;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return out;
}));

(cljs.core.async.map.cljs$lang$maxFixedArity = 3);

/**
 * Takes a collection of source channels and returns a channel which
 *   contains all values taken from them. The returned channel will be
 *   unbuffered by default, or a buf-or-n can be supplied. The channel
 *   will close after all the source channels have closed.
 */
cljs.core.async.merge = (function cljs$core$async$merge(var_args){
var G__43985 = arguments.length;
switch (G__43985) {
case 1:
return cljs.core.async.merge.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 2:
return cljs.core.async.merge.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.merge.cljs$core$IFn$_invoke$arity$1 = (function (chs){
return cljs.core.async.merge.call(null,chs,null);
}));

(cljs.core.async.merge.cljs$core$IFn$_invoke$arity$2 = (function (chs,buf_or_n){
var out = cljs.core.async.chan.call(null,buf_or_n);
var c__26800__auto___44039 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_44017){
var state_val_44018 = (state_44017[(1)]);
if((state_val_44018 === (7))){
var inst_43997 = (state_44017[(7)]);
var inst_43996 = (state_44017[(8)]);
var inst_43996__$1 = (state_44017[(2)]);
var inst_43997__$1 = cljs.core.nth.call(null,inst_43996__$1,(0),null);
var inst_43998 = cljs.core.nth.call(null,inst_43996__$1,(1),null);
var inst_43999 = (inst_43997__$1 == null);
var state_44017__$1 = (function (){var statearr_44019 = state_44017;
(statearr_44019[(9)] = inst_43998);

(statearr_44019[(7)] = inst_43997__$1);

(statearr_44019[(8)] = inst_43996__$1);

return statearr_44019;
})();
if(cljs.core.truth_(inst_43999)){
var statearr_44020_44040 = state_44017__$1;
(statearr_44020_44040[(1)] = (8));

} else {
var statearr_44021_44041 = state_44017__$1;
(statearr_44021_44041[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44018 === (1))){
var inst_43986 = cljs.core.vec.call(null,chs);
var inst_43987 = inst_43986;
var state_44017__$1 = (function (){var statearr_44022 = state_44017;
(statearr_44022[(10)] = inst_43987);

return statearr_44022;
})();
var statearr_44023_44042 = state_44017__$1;
(statearr_44023_44042[(2)] = null);

(statearr_44023_44042[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44018 === (4))){
var inst_43987 = (state_44017[(10)]);
var state_44017__$1 = state_44017;
return cljs.core.async.ioc_alts_BANG_.call(null,state_44017__$1,(7),inst_43987);
} else {
if((state_val_44018 === (6))){
var inst_44013 = (state_44017[(2)]);
var state_44017__$1 = state_44017;
var statearr_44024_44043 = state_44017__$1;
(statearr_44024_44043[(2)] = inst_44013);

(statearr_44024_44043[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44018 === (3))){
var inst_44015 = (state_44017[(2)]);
var state_44017__$1 = state_44017;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_44017__$1,inst_44015);
} else {
if((state_val_44018 === (2))){
var inst_43987 = (state_44017[(10)]);
var inst_43989 = cljs.core.count.call(null,inst_43987);
var inst_43990 = (inst_43989 > (0));
var state_44017__$1 = state_44017;
if(cljs.core.truth_(inst_43990)){
var statearr_44026_44044 = state_44017__$1;
(statearr_44026_44044[(1)] = (4));

} else {
var statearr_44027_44045 = state_44017__$1;
(statearr_44027_44045[(1)] = (5));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44018 === (11))){
var inst_43987 = (state_44017[(10)]);
var inst_44006 = (state_44017[(2)]);
var tmp44025 = inst_43987;
var inst_43987__$1 = tmp44025;
var state_44017__$1 = (function (){var statearr_44028 = state_44017;
(statearr_44028[(10)] = inst_43987__$1);

(statearr_44028[(11)] = inst_44006);

return statearr_44028;
})();
var statearr_44029_44046 = state_44017__$1;
(statearr_44029_44046[(2)] = null);

(statearr_44029_44046[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44018 === (9))){
var inst_43997 = (state_44017[(7)]);
var state_44017__$1 = state_44017;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_44017__$1,(11),out,inst_43997);
} else {
if((state_val_44018 === (5))){
var inst_44011 = cljs.core.async.close_BANG_.call(null,out);
var state_44017__$1 = state_44017;
var statearr_44030_44047 = state_44017__$1;
(statearr_44030_44047[(2)] = inst_44011);

(statearr_44030_44047[(1)] = (6));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44018 === (10))){
var inst_44009 = (state_44017[(2)]);
var state_44017__$1 = state_44017;
var statearr_44031_44048 = state_44017__$1;
(statearr_44031_44048[(2)] = inst_44009);

(statearr_44031_44048[(1)] = (6));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44018 === (8))){
var inst_43987 = (state_44017[(10)]);
var inst_43998 = (state_44017[(9)]);
var inst_43997 = (state_44017[(7)]);
var inst_43996 = (state_44017[(8)]);
var inst_44001 = (function (){var cs = inst_43987;
var vec__43992 = inst_43996;
var v = inst_43997;
var c = inst_43998;
return (function (p1__43983_SHARP_){
return cljs.core.not_EQ_.call(null,c,p1__43983_SHARP_);
});
})();
var inst_44002 = cljs.core.filterv.call(null,inst_44001,inst_43987);
var inst_43987__$1 = inst_44002;
var state_44017__$1 = (function (){var statearr_44032 = state_44017;
(statearr_44032[(10)] = inst_43987__$1);

return statearr_44032;
})();
var statearr_44033_44049 = state_44017__$1;
(statearr_44033_44049[(2)] = null);

(statearr_44033_44049[(1)] = (2));


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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_44034 = [null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_44034[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_44034[(1)] = (1));

return statearr_44034;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_44017){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_44017);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e44035){if((e44035 instanceof Object)){
var ex__26709__auto__ = e44035;
var statearr_44036_44050 = state_44017;
(statearr_44036_44050[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_44017);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e44035;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__44051 = state_44017;
state_44017 = G__44051;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_44017){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_44017);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_44037 = f__26801__auto__.call(null);
(statearr_44037[(6)] = c__26800__auto___44039);

return statearr_44037;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return out;
}));

(cljs.core.async.merge.cljs$lang$maxFixedArity = 2);

/**
 * Returns a channel containing the single (collection) result of the
 *   items taken from the channel conjoined to the supplied
 *   collection. ch must close before into produces a result.
 */
cljs.core.async.into = (function cljs$core$async$into(coll,ch){
return cljs.core.async.reduce.call(null,cljs.core.conj,coll,ch);
});
/**
 * Returns a channel that will return, at most, n items from ch. After n items
 * have been returned, or ch has been closed, the return chanel will close.
 * 
 *   The output channel is unbuffered by default, unless buf-or-n is given.
 */
cljs.core.async.take = (function cljs$core$async$take(var_args){
var G__44053 = arguments.length;
switch (G__44053) {
case 2:
return cljs.core.async.take.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.take.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.take.cljs$core$IFn$_invoke$arity$2 = (function (n,ch){
return cljs.core.async.take.call(null,n,ch,null);
}));

(cljs.core.async.take.cljs$core$IFn$_invoke$arity$3 = (function (n,ch,buf_or_n){
var out = cljs.core.async.chan.call(null,buf_or_n);
var c__26800__auto___44098 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_44077){
var state_val_44078 = (state_44077[(1)]);
if((state_val_44078 === (7))){
var inst_44059 = (state_44077[(7)]);
var inst_44059__$1 = (state_44077[(2)]);
var inst_44060 = (inst_44059__$1 == null);
var inst_44061 = cljs.core.not.call(null,inst_44060);
var state_44077__$1 = (function (){var statearr_44079 = state_44077;
(statearr_44079[(7)] = inst_44059__$1);

return statearr_44079;
})();
if(inst_44061){
var statearr_44080_44099 = state_44077__$1;
(statearr_44080_44099[(1)] = (8));

} else {
var statearr_44081_44100 = state_44077__$1;
(statearr_44081_44100[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44078 === (1))){
var inst_44054 = (0);
var state_44077__$1 = (function (){var statearr_44082 = state_44077;
(statearr_44082[(8)] = inst_44054);

return statearr_44082;
})();
var statearr_44083_44101 = state_44077__$1;
(statearr_44083_44101[(2)] = null);

(statearr_44083_44101[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44078 === (4))){
var state_44077__$1 = state_44077;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_44077__$1,(7),ch);
} else {
if((state_val_44078 === (6))){
var inst_44072 = (state_44077[(2)]);
var state_44077__$1 = state_44077;
var statearr_44084_44102 = state_44077__$1;
(statearr_44084_44102[(2)] = inst_44072);

(statearr_44084_44102[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44078 === (3))){
var inst_44074 = (state_44077[(2)]);
var inst_44075 = cljs.core.async.close_BANG_.call(null,out);
var state_44077__$1 = (function (){var statearr_44085 = state_44077;
(statearr_44085[(9)] = inst_44074);

return statearr_44085;
})();
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_44077__$1,inst_44075);
} else {
if((state_val_44078 === (2))){
var inst_44054 = (state_44077[(8)]);
var inst_44056 = (inst_44054 < n);
var state_44077__$1 = state_44077;
if(cljs.core.truth_(inst_44056)){
var statearr_44086_44103 = state_44077__$1;
(statearr_44086_44103[(1)] = (4));

} else {
var statearr_44087_44104 = state_44077__$1;
(statearr_44087_44104[(1)] = (5));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44078 === (11))){
var inst_44054 = (state_44077[(8)]);
var inst_44064 = (state_44077[(2)]);
var inst_44065 = (inst_44054 + (1));
var inst_44054__$1 = inst_44065;
var state_44077__$1 = (function (){var statearr_44088 = state_44077;
(statearr_44088[(10)] = inst_44064);

(statearr_44088[(8)] = inst_44054__$1);

return statearr_44088;
})();
var statearr_44089_44105 = state_44077__$1;
(statearr_44089_44105[(2)] = null);

(statearr_44089_44105[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44078 === (9))){
var state_44077__$1 = state_44077;
var statearr_44090_44106 = state_44077__$1;
(statearr_44090_44106[(2)] = null);

(statearr_44090_44106[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44078 === (5))){
var state_44077__$1 = state_44077;
var statearr_44091_44107 = state_44077__$1;
(statearr_44091_44107[(2)] = null);

(statearr_44091_44107[(1)] = (6));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44078 === (10))){
var inst_44069 = (state_44077[(2)]);
var state_44077__$1 = state_44077;
var statearr_44092_44108 = state_44077__$1;
(statearr_44092_44108[(2)] = inst_44069);

(statearr_44092_44108[(1)] = (6));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44078 === (8))){
var inst_44059 = (state_44077[(7)]);
var state_44077__$1 = state_44077;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_44077__$1,(11),out,inst_44059);
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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_44093 = [null,null,null,null,null,null,null,null,null,null,null];
(statearr_44093[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_44093[(1)] = (1));

return statearr_44093;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_44077){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_44077);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e44094){if((e44094 instanceof Object)){
var ex__26709__auto__ = e44094;
var statearr_44095_44109 = state_44077;
(statearr_44095_44109[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_44077);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e44094;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__44110 = state_44077;
state_44077 = G__44110;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_44077){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_44077);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_44096 = f__26801__auto__.call(null);
(statearr_44096[(6)] = c__26800__auto___44098);

return statearr_44096;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return out;
}));

(cljs.core.async.take.cljs$lang$maxFixedArity = 3);

/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.map_LT_ = (function cljs$core$async$map_LT_(f,ch){
if((typeof cljs !== 'undefined') && (typeof cljs.core !== 'undefined') && (typeof cljs.core.async !== 'undefined') && (typeof cljs.core.async.t_cljs$core$async44112 !== 'undefined')){
} else {

/**
* @constructor
 * @implements {cljs.core.async.impl.protocols.Channel}
 * @implements {cljs.core.async.impl.protocols.WritePort}
 * @implements {cljs.core.async.impl.protocols.ReadPort}
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.IWithMeta}
*/
cljs.core.async.t_cljs$core$async44112 = (function (f,ch,meta44113){
this.f = f;
this.ch = ch;
this.meta44113 = meta44113;
this.cljs$lang$protocol_mask$partition0$ = 393216;
this.cljs$lang$protocol_mask$partition1$ = 0;
});
(cljs.core.async.t_cljs$core$async44112.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (_44114,meta44113__$1){
var self__ = this;
var _44114__$1 = this;
return (new cljs.core.async.t_cljs$core$async44112(self__.f,self__.ch,meta44113__$1));
}));

(cljs.core.async.t_cljs$core$async44112.prototype.cljs$core$IMeta$_meta$arity$1 = (function (_44114){
var self__ = this;
var _44114__$1 = this;
return self__.meta44113;
}));

(cljs.core.async.t_cljs$core$async44112.prototype.cljs$core$async$impl$protocols$Channel$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async44112.prototype.cljs$core$async$impl$protocols$Channel$close_BANG_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return cljs.core.async.impl.protocols.close_BANG_.call(null,self__.ch);
}));

(cljs.core.async.t_cljs$core$async44112.prototype.cljs$core$async$impl$protocols$Channel$closed_QMARK_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return cljs.core.async.impl.protocols.closed_QMARK_.call(null,self__.ch);
}));

(cljs.core.async.t_cljs$core$async44112.prototype.cljs$core$async$impl$protocols$ReadPort$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async44112.prototype.cljs$core$async$impl$protocols$ReadPort$take_BANG_$arity$2 = (function (_,fn1){
var self__ = this;
var ___$1 = this;
var ret = cljs.core.async.impl.protocols.take_BANG_.call(null,self__.ch,(function (){
if((typeof cljs !== 'undefined') && (typeof cljs.core !== 'undefined') && (typeof cljs.core.async !== 'undefined') && (typeof cljs.core.async.t_cljs$core$async44115 !== 'undefined')){
} else {

/**
* @constructor
 * @implements {cljs.core.async.impl.protocols.Handler}
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.IWithMeta}
*/
cljs.core.async.t_cljs$core$async44115 = (function (f,ch,meta44113,_,fn1,meta44116){
this.f = f;
this.ch = ch;
this.meta44113 = meta44113;
this._ = _;
this.fn1 = fn1;
this.meta44116 = meta44116;
this.cljs$lang$protocol_mask$partition0$ = 393216;
this.cljs$lang$protocol_mask$partition1$ = 0;
});
(cljs.core.async.t_cljs$core$async44115.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (_44117,meta44116__$1){
var self__ = this;
var _44117__$1 = this;
return (new cljs.core.async.t_cljs$core$async44115(self__.f,self__.ch,self__.meta44113,self__._,self__.fn1,meta44116__$1));
}));

(cljs.core.async.t_cljs$core$async44115.prototype.cljs$core$IMeta$_meta$arity$1 = (function (_44117){
var self__ = this;
var _44117__$1 = this;
return self__.meta44116;
}));

(cljs.core.async.t_cljs$core$async44115.prototype.cljs$core$async$impl$protocols$Handler$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async44115.prototype.cljs$core$async$impl$protocols$Handler$active_QMARK_$arity$1 = (function (___$1){
var self__ = this;
var ___$2 = this;
return cljs.core.async.impl.protocols.active_QMARK_.call(null,self__.fn1);
}));

(cljs.core.async.t_cljs$core$async44115.prototype.cljs$core$async$impl$protocols$Handler$blockable_QMARK_$arity$1 = (function (___$1){
var self__ = this;
var ___$2 = this;
return true;
}));

(cljs.core.async.t_cljs$core$async44115.prototype.cljs$core$async$impl$protocols$Handler$commit$arity$1 = (function (___$1){
var self__ = this;
var ___$2 = this;
var f1 = cljs.core.async.impl.protocols.commit.call(null,self__.fn1);
return (function (p1__44111_SHARP_){
return f1.call(null,(((p1__44111_SHARP_ == null))?null:self__.f.call(null,p1__44111_SHARP_)));
});
}));

(cljs.core.async.t_cljs$core$async44115.getBasis = (function (){
return new cljs.core.PersistentVector(null, 6, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"f","f",43394975,null),new cljs.core.Symbol(null,"ch","ch",1085813622,null),new cljs.core.Symbol(null,"meta44113","meta44113",-1169609899,null),cljs.core.with_meta(new cljs.core.Symbol(null,"_","_",-1201019570,null),new cljs.core.PersistentArrayMap(null, 1, [new cljs.core.Keyword(null,"tag","tag",-1290361223),new cljs.core.Symbol("cljs.core.async","t_cljs$core$async44112","cljs.core.async/t_cljs$core$async44112",-1459039127,null)], null)),new cljs.core.Symbol(null,"fn1","fn1",895834444,null),new cljs.core.Symbol(null,"meta44116","meta44116",1032598161,null)], null);
}));

(cljs.core.async.t_cljs$core$async44115.cljs$lang$type = true);

(cljs.core.async.t_cljs$core$async44115.cljs$lang$ctorStr = "cljs.core.async/t_cljs$core$async44115");

(cljs.core.async.t_cljs$core$async44115.cljs$lang$ctorPrWriter = (function (this__4369__auto__,writer__4370__auto__,opt__4371__auto__){
return cljs.core._write.call(null,writer__4370__auto__,"cljs.core.async/t_cljs$core$async44115");
}));

/**
 * Positional factory function for cljs.core.async/t_cljs$core$async44115.
 */
cljs.core.async.__GT_t_cljs$core$async44115 = (function cljs$core$async$map_LT__$___GT_t_cljs$core$async44115(f__$1,ch__$1,meta44113__$1,___$2,fn1__$1,meta44116){
return (new cljs.core.async.t_cljs$core$async44115(f__$1,ch__$1,meta44113__$1,___$2,fn1__$1,meta44116));
});

}

return (new cljs.core.async.t_cljs$core$async44115(self__.f,self__.ch,self__.meta44113,___$1,fn1,cljs.core.PersistentArrayMap.EMPTY));
})()
);
if(cljs.core.truth_((function (){var and__4115__auto__ = ret;
if(cljs.core.truth_(and__4115__auto__)){
return (!((cljs.core.deref.call(null,ret) == null)));
} else {
return and__4115__auto__;
}
})())){
return cljs.core.async.impl.channels.box.call(null,self__.f.call(null,cljs.core.deref.call(null,ret)));
} else {
return ret;
}
}));

(cljs.core.async.t_cljs$core$async44112.prototype.cljs$core$async$impl$protocols$WritePort$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async44112.prototype.cljs$core$async$impl$protocols$WritePort$put_BANG_$arity$3 = (function (_,val,fn1){
var self__ = this;
var ___$1 = this;
return cljs.core.async.impl.protocols.put_BANG_.call(null,self__.ch,val,fn1);
}));

(cljs.core.async.t_cljs$core$async44112.getBasis = (function (){
return new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"f","f",43394975,null),new cljs.core.Symbol(null,"ch","ch",1085813622,null),new cljs.core.Symbol(null,"meta44113","meta44113",-1169609899,null)], null);
}));

(cljs.core.async.t_cljs$core$async44112.cljs$lang$type = true);

(cljs.core.async.t_cljs$core$async44112.cljs$lang$ctorStr = "cljs.core.async/t_cljs$core$async44112");

(cljs.core.async.t_cljs$core$async44112.cljs$lang$ctorPrWriter = (function (this__4369__auto__,writer__4370__auto__,opt__4371__auto__){
return cljs.core._write.call(null,writer__4370__auto__,"cljs.core.async/t_cljs$core$async44112");
}));

/**
 * Positional factory function for cljs.core.async/t_cljs$core$async44112.
 */
cljs.core.async.__GT_t_cljs$core$async44112 = (function cljs$core$async$map_LT__$___GT_t_cljs$core$async44112(f__$1,ch__$1,meta44113){
return (new cljs.core.async.t_cljs$core$async44112(f__$1,ch__$1,meta44113));
});

}

return (new cljs.core.async.t_cljs$core$async44112(f,ch,cljs.core.PersistentArrayMap.EMPTY));
});
/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.map_GT_ = (function cljs$core$async$map_GT_(f,ch){
if((typeof cljs !== 'undefined') && (typeof cljs.core !== 'undefined') && (typeof cljs.core.async !== 'undefined') && (typeof cljs.core.async.t_cljs$core$async44118 !== 'undefined')){
} else {

/**
* @constructor
 * @implements {cljs.core.async.impl.protocols.Channel}
 * @implements {cljs.core.async.impl.protocols.WritePort}
 * @implements {cljs.core.async.impl.protocols.ReadPort}
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.IWithMeta}
*/
cljs.core.async.t_cljs$core$async44118 = (function (f,ch,meta44119){
this.f = f;
this.ch = ch;
this.meta44119 = meta44119;
this.cljs$lang$protocol_mask$partition0$ = 393216;
this.cljs$lang$protocol_mask$partition1$ = 0;
});
(cljs.core.async.t_cljs$core$async44118.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (_44120,meta44119__$1){
var self__ = this;
var _44120__$1 = this;
return (new cljs.core.async.t_cljs$core$async44118(self__.f,self__.ch,meta44119__$1));
}));

(cljs.core.async.t_cljs$core$async44118.prototype.cljs$core$IMeta$_meta$arity$1 = (function (_44120){
var self__ = this;
var _44120__$1 = this;
return self__.meta44119;
}));

(cljs.core.async.t_cljs$core$async44118.prototype.cljs$core$async$impl$protocols$Channel$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async44118.prototype.cljs$core$async$impl$protocols$Channel$close_BANG_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return cljs.core.async.impl.protocols.close_BANG_.call(null,self__.ch);
}));

(cljs.core.async.t_cljs$core$async44118.prototype.cljs$core$async$impl$protocols$ReadPort$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async44118.prototype.cljs$core$async$impl$protocols$ReadPort$take_BANG_$arity$2 = (function (_,fn1){
var self__ = this;
var ___$1 = this;
return cljs.core.async.impl.protocols.take_BANG_.call(null,self__.ch,fn1);
}));

(cljs.core.async.t_cljs$core$async44118.prototype.cljs$core$async$impl$protocols$WritePort$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async44118.prototype.cljs$core$async$impl$protocols$WritePort$put_BANG_$arity$3 = (function (_,val,fn1){
var self__ = this;
var ___$1 = this;
return cljs.core.async.impl.protocols.put_BANG_.call(null,self__.ch,self__.f.call(null,val),fn1);
}));

(cljs.core.async.t_cljs$core$async44118.getBasis = (function (){
return new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"f","f",43394975,null),new cljs.core.Symbol(null,"ch","ch",1085813622,null),new cljs.core.Symbol(null,"meta44119","meta44119",-550625081,null)], null);
}));

(cljs.core.async.t_cljs$core$async44118.cljs$lang$type = true);

(cljs.core.async.t_cljs$core$async44118.cljs$lang$ctorStr = "cljs.core.async/t_cljs$core$async44118");

(cljs.core.async.t_cljs$core$async44118.cljs$lang$ctorPrWriter = (function (this__4369__auto__,writer__4370__auto__,opt__4371__auto__){
return cljs.core._write.call(null,writer__4370__auto__,"cljs.core.async/t_cljs$core$async44118");
}));

/**
 * Positional factory function for cljs.core.async/t_cljs$core$async44118.
 */
cljs.core.async.__GT_t_cljs$core$async44118 = (function cljs$core$async$map_GT__$___GT_t_cljs$core$async44118(f__$1,ch__$1,meta44119){
return (new cljs.core.async.t_cljs$core$async44118(f__$1,ch__$1,meta44119));
});

}

return (new cljs.core.async.t_cljs$core$async44118(f,ch,cljs.core.PersistentArrayMap.EMPTY));
});
/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.filter_GT_ = (function cljs$core$async$filter_GT_(p,ch){
if((typeof cljs !== 'undefined') && (typeof cljs.core !== 'undefined') && (typeof cljs.core.async !== 'undefined') && (typeof cljs.core.async.t_cljs$core$async44121 !== 'undefined')){
} else {

/**
* @constructor
 * @implements {cljs.core.async.impl.protocols.Channel}
 * @implements {cljs.core.async.impl.protocols.WritePort}
 * @implements {cljs.core.async.impl.protocols.ReadPort}
 * @implements {cljs.core.IMeta}
 * @implements {cljs.core.IWithMeta}
*/
cljs.core.async.t_cljs$core$async44121 = (function (p,ch,meta44122){
this.p = p;
this.ch = ch;
this.meta44122 = meta44122;
this.cljs$lang$protocol_mask$partition0$ = 393216;
this.cljs$lang$protocol_mask$partition1$ = 0;
});
(cljs.core.async.t_cljs$core$async44121.prototype.cljs$core$IWithMeta$_with_meta$arity$2 = (function (_44123,meta44122__$1){
var self__ = this;
var _44123__$1 = this;
return (new cljs.core.async.t_cljs$core$async44121(self__.p,self__.ch,meta44122__$1));
}));

(cljs.core.async.t_cljs$core$async44121.prototype.cljs$core$IMeta$_meta$arity$1 = (function (_44123){
var self__ = this;
var _44123__$1 = this;
return self__.meta44122;
}));

(cljs.core.async.t_cljs$core$async44121.prototype.cljs$core$async$impl$protocols$Channel$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async44121.prototype.cljs$core$async$impl$protocols$Channel$close_BANG_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return cljs.core.async.impl.protocols.close_BANG_.call(null,self__.ch);
}));

(cljs.core.async.t_cljs$core$async44121.prototype.cljs$core$async$impl$protocols$Channel$closed_QMARK_$arity$1 = (function (_){
var self__ = this;
var ___$1 = this;
return cljs.core.async.impl.protocols.closed_QMARK_.call(null,self__.ch);
}));

(cljs.core.async.t_cljs$core$async44121.prototype.cljs$core$async$impl$protocols$ReadPort$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async44121.prototype.cljs$core$async$impl$protocols$ReadPort$take_BANG_$arity$2 = (function (_,fn1){
var self__ = this;
var ___$1 = this;
return cljs.core.async.impl.protocols.take_BANG_.call(null,self__.ch,fn1);
}));

(cljs.core.async.t_cljs$core$async44121.prototype.cljs$core$async$impl$protocols$WritePort$ = cljs.core.PROTOCOL_SENTINEL);

(cljs.core.async.t_cljs$core$async44121.prototype.cljs$core$async$impl$protocols$WritePort$put_BANG_$arity$3 = (function (_,val,fn1){
var self__ = this;
var ___$1 = this;
if(cljs.core.truth_(self__.p.call(null,val))){
return cljs.core.async.impl.protocols.put_BANG_.call(null,self__.ch,val,fn1);
} else {
return cljs.core.async.impl.channels.box.call(null,cljs.core.not.call(null,cljs.core.async.impl.protocols.closed_QMARK_.call(null,self__.ch)));
}
}));

(cljs.core.async.t_cljs$core$async44121.getBasis = (function (){
return new cljs.core.PersistentVector(null, 3, 5, cljs.core.PersistentVector.EMPTY_NODE, [new cljs.core.Symbol(null,"p","p",1791580836,null),new cljs.core.Symbol(null,"ch","ch",1085813622,null),new cljs.core.Symbol(null,"meta44122","meta44122",-1282878782,null)], null);
}));

(cljs.core.async.t_cljs$core$async44121.cljs$lang$type = true);

(cljs.core.async.t_cljs$core$async44121.cljs$lang$ctorStr = "cljs.core.async/t_cljs$core$async44121");

(cljs.core.async.t_cljs$core$async44121.cljs$lang$ctorPrWriter = (function (this__4369__auto__,writer__4370__auto__,opt__4371__auto__){
return cljs.core._write.call(null,writer__4370__auto__,"cljs.core.async/t_cljs$core$async44121");
}));

/**
 * Positional factory function for cljs.core.async/t_cljs$core$async44121.
 */
cljs.core.async.__GT_t_cljs$core$async44121 = (function cljs$core$async$filter_GT__$___GT_t_cljs$core$async44121(p__$1,ch__$1,meta44122){
return (new cljs.core.async.t_cljs$core$async44121(p__$1,ch__$1,meta44122));
});

}

return (new cljs.core.async.t_cljs$core$async44121(p,ch,cljs.core.PersistentArrayMap.EMPTY));
});
/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.remove_GT_ = (function cljs$core$async$remove_GT_(p,ch){
return cljs.core.async.filter_GT_.call(null,cljs.core.complement.call(null,p),ch);
});
/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.filter_LT_ = (function cljs$core$async$filter_LT_(var_args){
var G__44125 = arguments.length;
switch (G__44125) {
case 2:
return cljs.core.async.filter_LT_.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.filter_LT_.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.filter_LT_.cljs$core$IFn$_invoke$arity$2 = (function (p,ch){
return cljs.core.async.filter_LT_.call(null,p,ch,null);
}));

(cljs.core.async.filter_LT_.cljs$core$IFn$_invoke$arity$3 = (function (p,ch,buf_or_n){
var out = cljs.core.async.chan.call(null,buf_or_n);
var c__26800__auto___44165 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_44146){
var state_val_44147 = (state_44146[(1)]);
if((state_val_44147 === (7))){
var inst_44142 = (state_44146[(2)]);
var state_44146__$1 = state_44146;
var statearr_44148_44166 = state_44146__$1;
(statearr_44148_44166[(2)] = inst_44142);

(statearr_44148_44166[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44147 === (1))){
var state_44146__$1 = state_44146;
var statearr_44149_44167 = state_44146__$1;
(statearr_44149_44167[(2)] = null);

(statearr_44149_44167[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44147 === (4))){
var inst_44128 = (state_44146[(7)]);
var inst_44128__$1 = (state_44146[(2)]);
var inst_44129 = (inst_44128__$1 == null);
var state_44146__$1 = (function (){var statearr_44150 = state_44146;
(statearr_44150[(7)] = inst_44128__$1);

return statearr_44150;
})();
if(cljs.core.truth_(inst_44129)){
var statearr_44151_44168 = state_44146__$1;
(statearr_44151_44168[(1)] = (5));

} else {
var statearr_44152_44169 = state_44146__$1;
(statearr_44152_44169[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44147 === (6))){
var inst_44128 = (state_44146[(7)]);
var inst_44133 = p.call(null,inst_44128);
var state_44146__$1 = state_44146;
if(cljs.core.truth_(inst_44133)){
var statearr_44153_44170 = state_44146__$1;
(statearr_44153_44170[(1)] = (8));

} else {
var statearr_44154_44171 = state_44146__$1;
(statearr_44154_44171[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44147 === (3))){
var inst_44144 = (state_44146[(2)]);
var state_44146__$1 = state_44146;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_44146__$1,inst_44144);
} else {
if((state_val_44147 === (2))){
var state_44146__$1 = state_44146;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_44146__$1,(4),ch);
} else {
if((state_val_44147 === (11))){
var inst_44136 = (state_44146[(2)]);
var state_44146__$1 = state_44146;
var statearr_44155_44172 = state_44146__$1;
(statearr_44155_44172[(2)] = inst_44136);

(statearr_44155_44172[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44147 === (9))){
var state_44146__$1 = state_44146;
var statearr_44156_44173 = state_44146__$1;
(statearr_44156_44173[(2)] = null);

(statearr_44156_44173[(1)] = (10));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44147 === (5))){
var inst_44131 = cljs.core.async.close_BANG_.call(null,out);
var state_44146__$1 = state_44146;
var statearr_44157_44174 = state_44146__$1;
(statearr_44157_44174[(2)] = inst_44131);

(statearr_44157_44174[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44147 === (10))){
var inst_44139 = (state_44146[(2)]);
var state_44146__$1 = (function (){var statearr_44158 = state_44146;
(statearr_44158[(8)] = inst_44139);

return statearr_44158;
})();
var statearr_44159_44175 = state_44146__$1;
(statearr_44159_44175[(2)] = null);

(statearr_44159_44175[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44147 === (8))){
var inst_44128 = (state_44146[(7)]);
var state_44146__$1 = state_44146;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_44146__$1,(11),out,inst_44128);
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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_44160 = [null,null,null,null,null,null,null,null,null];
(statearr_44160[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_44160[(1)] = (1));

return statearr_44160;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_44146){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_44146);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e44161){if((e44161 instanceof Object)){
var ex__26709__auto__ = e44161;
var statearr_44162_44176 = state_44146;
(statearr_44162_44176[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_44146);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e44161;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__44177 = state_44146;
state_44146 = G__44177;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_44146){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_44146);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_44163 = f__26801__auto__.call(null);
(statearr_44163[(6)] = c__26800__auto___44165);

return statearr_44163;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return out;
}));

(cljs.core.async.filter_LT_.cljs$lang$maxFixedArity = 3);

/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.remove_LT_ = (function cljs$core$async$remove_LT_(var_args){
var G__44179 = arguments.length;
switch (G__44179) {
case 2:
return cljs.core.async.remove_LT_.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.remove_LT_.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.remove_LT_.cljs$core$IFn$_invoke$arity$2 = (function (p,ch){
return cljs.core.async.remove_LT_.call(null,p,ch,null);
}));

(cljs.core.async.remove_LT_.cljs$core$IFn$_invoke$arity$3 = (function (p,ch,buf_or_n){
return cljs.core.async.filter_LT_.call(null,cljs.core.complement.call(null,p),ch,buf_or_n);
}));

(cljs.core.async.remove_LT_.cljs$lang$maxFixedArity = 3);

cljs.core.async.mapcat_STAR_ = (function cljs$core$async$mapcat_STAR_(f,in$,out){
var c__26800__auto__ = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_44242){
var state_val_44243 = (state_44242[(1)]);
if((state_val_44243 === (7))){
var inst_44238 = (state_44242[(2)]);
var state_44242__$1 = state_44242;
var statearr_44244_44282 = state_44242__$1;
(statearr_44244_44282[(2)] = inst_44238);

(statearr_44244_44282[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (20))){
var inst_44208 = (state_44242[(7)]);
var inst_44219 = (state_44242[(2)]);
var inst_44220 = cljs.core.next.call(null,inst_44208);
var inst_44194 = inst_44220;
var inst_44195 = null;
var inst_44196 = (0);
var inst_44197 = (0);
var state_44242__$1 = (function (){var statearr_44245 = state_44242;
(statearr_44245[(8)] = inst_44194);

(statearr_44245[(9)] = inst_44197);

(statearr_44245[(10)] = inst_44196);

(statearr_44245[(11)] = inst_44219);

(statearr_44245[(12)] = inst_44195);

return statearr_44245;
})();
var statearr_44246_44283 = state_44242__$1;
(statearr_44246_44283[(2)] = null);

(statearr_44246_44283[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (1))){
var state_44242__$1 = state_44242;
var statearr_44247_44284 = state_44242__$1;
(statearr_44247_44284[(2)] = null);

(statearr_44247_44284[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (4))){
var inst_44183 = (state_44242[(13)]);
var inst_44183__$1 = (state_44242[(2)]);
var inst_44184 = (inst_44183__$1 == null);
var state_44242__$1 = (function (){var statearr_44248 = state_44242;
(statearr_44248[(13)] = inst_44183__$1);

return statearr_44248;
})();
if(cljs.core.truth_(inst_44184)){
var statearr_44249_44285 = state_44242__$1;
(statearr_44249_44285[(1)] = (5));

} else {
var statearr_44250_44286 = state_44242__$1;
(statearr_44250_44286[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (15))){
var state_44242__$1 = state_44242;
var statearr_44254_44287 = state_44242__$1;
(statearr_44254_44287[(2)] = null);

(statearr_44254_44287[(1)] = (16));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (21))){
var state_44242__$1 = state_44242;
var statearr_44255_44288 = state_44242__$1;
(statearr_44255_44288[(2)] = null);

(statearr_44255_44288[(1)] = (23));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (13))){
var inst_44194 = (state_44242[(8)]);
var inst_44197 = (state_44242[(9)]);
var inst_44196 = (state_44242[(10)]);
var inst_44195 = (state_44242[(12)]);
var inst_44204 = (state_44242[(2)]);
var inst_44205 = (inst_44197 + (1));
var tmp44251 = inst_44194;
var tmp44252 = inst_44196;
var tmp44253 = inst_44195;
var inst_44194__$1 = tmp44251;
var inst_44195__$1 = tmp44253;
var inst_44196__$1 = tmp44252;
var inst_44197__$1 = inst_44205;
var state_44242__$1 = (function (){var statearr_44256 = state_44242;
(statearr_44256[(8)] = inst_44194__$1);

(statearr_44256[(9)] = inst_44197__$1);

(statearr_44256[(14)] = inst_44204);

(statearr_44256[(10)] = inst_44196__$1);

(statearr_44256[(12)] = inst_44195__$1);

return statearr_44256;
})();
var statearr_44257_44289 = state_44242__$1;
(statearr_44257_44289[(2)] = null);

(statearr_44257_44289[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (22))){
var state_44242__$1 = state_44242;
var statearr_44258_44290 = state_44242__$1;
(statearr_44258_44290[(2)] = null);

(statearr_44258_44290[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (6))){
var inst_44183 = (state_44242[(13)]);
var inst_44192 = f.call(null,inst_44183);
var inst_44193 = cljs.core.seq.call(null,inst_44192);
var inst_44194 = inst_44193;
var inst_44195 = null;
var inst_44196 = (0);
var inst_44197 = (0);
var state_44242__$1 = (function (){var statearr_44259 = state_44242;
(statearr_44259[(8)] = inst_44194);

(statearr_44259[(9)] = inst_44197);

(statearr_44259[(10)] = inst_44196);

(statearr_44259[(12)] = inst_44195);

return statearr_44259;
})();
var statearr_44260_44291 = state_44242__$1;
(statearr_44260_44291[(2)] = null);

(statearr_44260_44291[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (17))){
var inst_44208 = (state_44242[(7)]);
var inst_44212 = cljs.core.chunk_first.call(null,inst_44208);
var inst_44213 = cljs.core.chunk_rest.call(null,inst_44208);
var inst_44214 = cljs.core.count.call(null,inst_44212);
var inst_44194 = inst_44213;
var inst_44195 = inst_44212;
var inst_44196 = inst_44214;
var inst_44197 = (0);
var state_44242__$1 = (function (){var statearr_44261 = state_44242;
(statearr_44261[(8)] = inst_44194);

(statearr_44261[(9)] = inst_44197);

(statearr_44261[(10)] = inst_44196);

(statearr_44261[(12)] = inst_44195);

return statearr_44261;
})();
var statearr_44262_44292 = state_44242__$1;
(statearr_44262_44292[(2)] = null);

(statearr_44262_44292[(1)] = (8));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (3))){
var inst_44240 = (state_44242[(2)]);
var state_44242__$1 = state_44242;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_44242__$1,inst_44240);
} else {
if((state_val_44243 === (12))){
var inst_44228 = (state_44242[(2)]);
var state_44242__$1 = state_44242;
var statearr_44263_44293 = state_44242__$1;
(statearr_44263_44293[(2)] = inst_44228);

(statearr_44263_44293[(1)] = (9));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (2))){
var state_44242__$1 = state_44242;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_44242__$1,(4),in$);
} else {
if((state_val_44243 === (23))){
var inst_44236 = (state_44242[(2)]);
var state_44242__$1 = state_44242;
var statearr_44264_44294 = state_44242__$1;
(statearr_44264_44294[(2)] = inst_44236);

(statearr_44264_44294[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (19))){
var inst_44223 = (state_44242[(2)]);
var state_44242__$1 = state_44242;
var statearr_44265_44295 = state_44242__$1;
(statearr_44265_44295[(2)] = inst_44223);

(statearr_44265_44295[(1)] = (16));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (11))){
var inst_44194 = (state_44242[(8)]);
var inst_44208 = (state_44242[(7)]);
var inst_44208__$1 = cljs.core.seq.call(null,inst_44194);
var state_44242__$1 = (function (){var statearr_44266 = state_44242;
(statearr_44266[(7)] = inst_44208__$1);

return statearr_44266;
})();
if(inst_44208__$1){
var statearr_44267_44296 = state_44242__$1;
(statearr_44267_44296[(1)] = (14));

} else {
var statearr_44268_44297 = state_44242__$1;
(statearr_44268_44297[(1)] = (15));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (9))){
var inst_44230 = (state_44242[(2)]);
var inst_44231 = cljs.core.async.impl.protocols.closed_QMARK_.call(null,out);
var state_44242__$1 = (function (){var statearr_44269 = state_44242;
(statearr_44269[(15)] = inst_44230);

return statearr_44269;
})();
if(cljs.core.truth_(inst_44231)){
var statearr_44270_44298 = state_44242__$1;
(statearr_44270_44298[(1)] = (21));

} else {
var statearr_44271_44299 = state_44242__$1;
(statearr_44271_44299[(1)] = (22));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (5))){
var inst_44186 = cljs.core.async.close_BANG_.call(null,out);
var state_44242__$1 = state_44242;
var statearr_44272_44300 = state_44242__$1;
(statearr_44272_44300[(2)] = inst_44186);

(statearr_44272_44300[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (14))){
var inst_44208 = (state_44242[(7)]);
var inst_44210 = cljs.core.chunked_seq_QMARK_.call(null,inst_44208);
var state_44242__$1 = state_44242;
if(inst_44210){
var statearr_44273_44301 = state_44242__$1;
(statearr_44273_44301[(1)] = (17));

} else {
var statearr_44274_44302 = state_44242__$1;
(statearr_44274_44302[(1)] = (18));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (16))){
var inst_44226 = (state_44242[(2)]);
var state_44242__$1 = state_44242;
var statearr_44275_44303 = state_44242__$1;
(statearr_44275_44303[(2)] = inst_44226);

(statearr_44275_44303[(1)] = (12));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44243 === (10))){
var inst_44197 = (state_44242[(9)]);
var inst_44195 = (state_44242[(12)]);
var inst_44202 = cljs.core._nth.call(null,inst_44195,inst_44197);
var state_44242__$1 = state_44242;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_44242__$1,(13),out,inst_44202);
} else {
if((state_val_44243 === (18))){
var inst_44208 = (state_44242[(7)]);
var inst_44217 = cljs.core.first.call(null,inst_44208);
var state_44242__$1 = state_44242;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_44242__$1,(20),out,inst_44217);
} else {
if((state_val_44243 === (8))){
var inst_44197 = (state_44242[(9)]);
var inst_44196 = (state_44242[(10)]);
var inst_44199 = (inst_44197 < inst_44196);
var inst_44200 = inst_44199;
var state_44242__$1 = state_44242;
if(cljs.core.truth_(inst_44200)){
var statearr_44276_44304 = state_44242__$1;
(statearr_44276_44304[(1)] = (10));

} else {
var statearr_44277_44305 = state_44242__$1;
(statearr_44277_44305[(1)] = (11));

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
});
return (function() {
var cljs$core$async$mapcat_STAR__$_state_machine__26706__auto__ = null;
var cljs$core$async$mapcat_STAR__$_state_machine__26706__auto____0 = (function (){
var statearr_44278 = [null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_44278[(0)] = cljs$core$async$mapcat_STAR__$_state_machine__26706__auto__);

(statearr_44278[(1)] = (1));

return statearr_44278;
});
var cljs$core$async$mapcat_STAR__$_state_machine__26706__auto____1 = (function (state_44242){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_44242);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e44279){if((e44279 instanceof Object)){
var ex__26709__auto__ = e44279;
var statearr_44280_44306 = state_44242;
(statearr_44280_44306[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_44242);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e44279;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__44307 = state_44242;
state_44242 = G__44307;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$mapcat_STAR__$_state_machine__26706__auto__ = function(state_44242){
switch(arguments.length){
case 0:
return cljs$core$async$mapcat_STAR__$_state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$mapcat_STAR__$_state_machine__26706__auto____1.call(this,state_44242);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$mapcat_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$mapcat_STAR__$_state_machine__26706__auto____0;
cljs$core$async$mapcat_STAR__$_state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$mapcat_STAR__$_state_machine__26706__auto____1;
return cljs$core$async$mapcat_STAR__$_state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_44281 = f__26801__auto__.call(null);
(statearr_44281[(6)] = c__26800__auto__);

return statearr_44281;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));

return c__26800__auto__;
});
/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.mapcat_LT_ = (function cljs$core$async$mapcat_LT_(var_args){
var G__44309 = arguments.length;
switch (G__44309) {
case 2:
return cljs.core.async.mapcat_LT_.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.mapcat_LT_.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.mapcat_LT_.cljs$core$IFn$_invoke$arity$2 = (function (f,in$){
return cljs.core.async.mapcat_LT_.call(null,f,in$,null);
}));

(cljs.core.async.mapcat_LT_.cljs$core$IFn$_invoke$arity$3 = (function (f,in$,buf_or_n){
var out = cljs.core.async.chan.call(null,buf_or_n);
cljs.core.async.mapcat_STAR_.call(null,f,in$,out);

return out;
}));

(cljs.core.async.mapcat_LT_.cljs$lang$maxFixedArity = 3);

/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.mapcat_GT_ = (function cljs$core$async$mapcat_GT_(var_args){
var G__44312 = arguments.length;
switch (G__44312) {
case 2:
return cljs.core.async.mapcat_GT_.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.mapcat_GT_.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.mapcat_GT_.cljs$core$IFn$_invoke$arity$2 = (function (f,out){
return cljs.core.async.mapcat_GT_.call(null,f,out,null);
}));

(cljs.core.async.mapcat_GT_.cljs$core$IFn$_invoke$arity$3 = (function (f,out,buf_or_n){
var in$ = cljs.core.async.chan.call(null,buf_or_n);
cljs.core.async.mapcat_STAR_.call(null,f,in$,out);

return in$;
}));

(cljs.core.async.mapcat_GT_.cljs$lang$maxFixedArity = 3);

/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.unique = (function cljs$core$async$unique(var_args){
var G__44315 = arguments.length;
switch (G__44315) {
case 1:
return cljs.core.async.unique.cljs$core$IFn$_invoke$arity$1((arguments[(0)]));

break;
case 2:
return cljs.core.async.unique.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.unique.cljs$core$IFn$_invoke$arity$1 = (function (ch){
return cljs.core.async.unique.call(null,ch,null);
}));

(cljs.core.async.unique.cljs$core$IFn$_invoke$arity$2 = (function (ch,buf_or_n){
var out = cljs.core.async.chan.call(null,buf_or_n);
var c__26800__auto___44362 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_44339){
var state_val_44340 = (state_44339[(1)]);
if((state_val_44340 === (7))){
var inst_44334 = (state_44339[(2)]);
var state_44339__$1 = state_44339;
var statearr_44341_44363 = state_44339__$1;
(statearr_44341_44363[(2)] = inst_44334);

(statearr_44341_44363[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44340 === (1))){
var inst_44316 = null;
var state_44339__$1 = (function (){var statearr_44342 = state_44339;
(statearr_44342[(7)] = inst_44316);

return statearr_44342;
})();
var statearr_44343_44364 = state_44339__$1;
(statearr_44343_44364[(2)] = null);

(statearr_44343_44364[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44340 === (4))){
var inst_44319 = (state_44339[(8)]);
var inst_44319__$1 = (state_44339[(2)]);
var inst_44320 = (inst_44319__$1 == null);
var inst_44321 = cljs.core.not.call(null,inst_44320);
var state_44339__$1 = (function (){var statearr_44344 = state_44339;
(statearr_44344[(8)] = inst_44319__$1);

return statearr_44344;
})();
if(inst_44321){
var statearr_44345_44365 = state_44339__$1;
(statearr_44345_44365[(1)] = (5));

} else {
var statearr_44346_44366 = state_44339__$1;
(statearr_44346_44366[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44340 === (6))){
var state_44339__$1 = state_44339;
var statearr_44347_44367 = state_44339__$1;
(statearr_44347_44367[(2)] = null);

(statearr_44347_44367[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44340 === (3))){
var inst_44336 = (state_44339[(2)]);
var inst_44337 = cljs.core.async.close_BANG_.call(null,out);
var state_44339__$1 = (function (){var statearr_44348 = state_44339;
(statearr_44348[(9)] = inst_44336);

return statearr_44348;
})();
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_44339__$1,inst_44337);
} else {
if((state_val_44340 === (2))){
var state_44339__$1 = state_44339;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_44339__$1,(4),ch);
} else {
if((state_val_44340 === (11))){
var inst_44319 = (state_44339[(8)]);
var inst_44328 = (state_44339[(2)]);
var inst_44316 = inst_44319;
var state_44339__$1 = (function (){var statearr_44349 = state_44339;
(statearr_44349[(7)] = inst_44316);

(statearr_44349[(10)] = inst_44328);

return statearr_44349;
})();
var statearr_44350_44368 = state_44339__$1;
(statearr_44350_44368[(2)] = null);

(statearr_44350_44368[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44340 === (9))){
var inst_44319 = (state_44339[(8)]);
var state_44339__$1 = state_44339;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_44339__$1,(11),out,inst_44319);
} else {
if((state_val_44340 === (5))){
var inst_44319 = (state_44339[(8)]);
var inst_44316 = (state_44339[(7)]);
var inst_44323 = cljs.core._EQ_.call(null,inst_44319,inst_44316);
var state_44339__$1 = state_44339;
if(inst_44323){
var statearr_44352_44369 = state_44339__$1;
(statearr_44352_44369[(1)] = (8));

} else {
var statearr_44353_44370 = state_44339__$1;
(statearr_44353_44370[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44340 === (10))){
var inst_44331 = (state_44339[(2)]);
var state_44339__$1 = state_44339;
var statearr_44354_44371 = state_44339__$1;
(statearr_44354_44371[(2)] = inst_44331);

(statearr_44354_44371[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44340 === (8))){
var inst_44316 = (state_44339[(7)]);
var tmp44351 = inst_44316;
var inst_44316__$1 = tmp44351;
var state_44339__$1 = (function (){var statearr_44355 = state_44339;
(statearr_44355[(7)] = inst_44316__$1);

return statearr_44355;
})();
var statearr_44356_44372 = state_44339__$1;
(statearr_44356_44372[(2)] = null);

(statearr_44356_44372[(1)] = (2));


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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_44357 = [null,null,null,null,null,null,null,null,null,null,null];
(statearr_44357[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_44357[(1)] = (1));

return statearr_44357;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_44339){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_44339);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e44358){if((e44358 instanceof Object)){
var ex__26709__auto__ = e44358;
var statearr_44359_44373 = state_44339;
(statearr_44359_44373[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_44339);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e44358;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__44374 = state_44339;
state_44339 = G__44374;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_44339){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_44339);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_44360 = f__26801__auto__.call(null);
(statearr_44360[(6)] = c__26800__auto___44362);

return statearr_44360;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return out;
}));

(cljs.core.async.unique.cljs$lang$maxFixedArity = 2);

/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.partition = (function cljs$core$async$partition(var_args){
var G__44376 = arguments.length;
switch (G__44376) {
case 2:
return cljs.core.async.partition.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.partition.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.partition.cljs$core$IFn$_invoke$arity$2 = (function (n,ch){
return cljs.core.async.partition.call(null,n,ch,null);
}));

(cljs.core.async.partition.cljs$core$IFn$_invoke$arity$3 = (function (n,ch,buf_or_n){
var out = cljs.core.async.chan.call(null,buf_or_n);
var c__26800__auto___44442 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_44414){
var state_val_44415 = (state_44414[(1)]);
if((state_val_44415 === (7))){
var inst_44410 = (state_44414[(2)]);
var state_44414__$1 = state_44414;
var statearr_44416_44443 = state_44414__$1;
(statearr_44416_44443[(2)] = inst_44410);

(statearr_44416_44443[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44415 === (1))){
var inst_44377 = (new Array(n));
var inst_44378 = inst_44377;
var inst_44379 = (0);
var state_44414__$1 = (function (){var statearr_44417 = state_44414;
(statearr_44417[(7)] = inst_44378);

(statearr_44417[(8)] = inst_44379);

return statearr_44417;
})();
var statearr_44418_44444 = state_44414__$1;
(statearr_44418_44444[(2)] = null);

(statearr_44418_44444[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44415 === (4))){
var inst_44382 = (state_44414[(9)]);
var inst_44382__$1 = (state_44414[(2)]);
var inst_44383 = (inst_44382__$1 == null);
var inst_44384 = cljs.core.not.call(null,inst_44383);
var state_44414__$1 = (function (){var statearr_44419 = state_44414;
(statearr_44419[(9)] = inst_44382__$1);

return statearr_44419;
})();
if(inst_44384){
var statearr_44420_44445 = state_44414__$1;
(statearr_44420_44445[(1)] = (5));

} else {
var statearr_44421_44446 = state_44414__$1;
(statearr_44421_44446[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44415 === (15))){
var inst_44404 = (state_44414[(2)]);
var state_44414__$1 = state_44414;
var statearr_44422_44447 = state_44414__$1;
(statearr_44422_44447[(2)] = inst_44404);

(statearr_44422_44447[(1)] = (14));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44415 === (13))){
var state_44414__$1 = state_44414;
var statearr_44423_44448 = state_44414__$1;
(statearr_44423_44448[(2)] = null);

(statearr_44423_44448[(1)] = (14));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44415 === (6))){
var inst_44379 = (state_44414[(8)]);
var inst_44400 = (inst_44379 > (0));
var state_44414__$1 = state_44414;
if(cljs.core.truth_(inst_44400)){
var statearr_44424_44449 = state_44414__$1;
(statearr_44424_44449[(1)] = (12));

} else {
var statearr_44425_44450 = state_44414__$1;
(statearr_44425_44450[(1)] = (13));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44415 === (3))){
var inst_44412 = (state_44414[(2)]);
var state_44414__$1 = state_44414;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_44414__$1,inst_44412);
} else {
if((state_val_44415 === (12))){
var inst_44378 = (state_44414[(7)]);
var inst_44402 = cljs.core.vec.call(null,inst_44378);
var state_44414__$1 = state_44414;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_44414__$1,(15),out,inst_44402);
} else {
if((state_val_44415 === (2))){
var state_44414__$1 = state_44414;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_44414__$1,(4),ch);
} else {
if((state_val_44415 === (11))){
var inst_44394 = (state_44414[(2)]);
var inst_44395 = (new Array(n));
var inst_44378 = inst_44395;
var inst_44379 = (0);
var state_44414__$1 = (function (){var statearr_44426 = state_44414;
(statearr_44426[(7)] = inst_44378);

(statearr_44426[(10)] = inst_44394);

(statearr_44426[(8)] = inst_44379);

return statearr_44426;
})();
var statearr_44427_44451 = state_44414__$1;
(statearr_44427_44451[(2)] = null);

(statearr_44427_44451[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44415 === (9))){
var inst_44378 = (state_44414[(7)]);
var inst_44392 = cljs.core.vec.call(null,inst_44378);
var state_44414__$1 = state_44414;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_44414__$1,(11),out,inst_44392);
} else {
if((state_val_44415 === (5))){
var inst_44378 = (state_44414[(7)]);
var inst_44382 = (state_44414[(9)]);
var inst_44379 = (state_44414[(8)]);
var inst_44387 = (state_44414[(11)]);
var inst_44386 = (inst_44378[inst_44379] = inst_44382);
var inst_44387__$1 = (inst_44379 + (1));
var inst_44388 = (inst_44387__$1 < n);
var state_44414__$1 = (function (){var statearr_44428 = state_44414;
(statearr_44428[(12)] = inst_44386);

(statearr_44428[(11)] = inst_44387__$1);

return statearr_44428;
})();
if(cljs.core.truth_(inst_44388)){
var statearr_44429_44452 = state_44414__$1;
(statearr_44429_44452[(1)] = (8));

} else {
var statearr_44430_44453 = state_44414__$1;
(statearr_44430_44453[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44415 === (14))){
var inst_44407 = (state_44414[(2)]);
var inst_44408 = cljs.core.async.close_BANG_.call(null,out);
var state_44414__$1 = (function (){var statearr_44432 = state_44414;
(statearr_44432[(13)] = inst_44407);

return statearr_44432;
})();
var statearr_44433_44454 = state_44414__$1;
(statearr_44433_44454[(2)] = inst_44408);

(statearr_44433_44454[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44415 === (10))){
var inst_44398 = (state_44414[(2)]);
var state_44414__$1 = state_44414;
var statearr_44434_44455 = state_44414__$1;
(statearr_44434_44455[(2)] = inst_44398);

(statearr_44434_44455[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44415 === (8))){
var inst_44378 = (state_44414[(7)]);
var inst_44387 = (state_44414[(11)]);
var tmp44431 = inst_44378;
var inst_44378__$1 = tmp44431;
var inst_44379 = inst_44387;
var state_44414__$1 = (function (){var statearr_44435 = state_44414;
(statearr_44435[(7)] = inst_44378__$1);

(statearr_44435[(8)] = inst_44379);

return statearr_44435;
})();
var statearr_44436_44456 = state_44414__$1;
(statearr_44436_44456[(2)] = null);

(statearr_44436_44456[(1)] = (2));


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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_44437 = [null,null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_44437[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_44437[(1)] = (1));

return statearr_44437;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_44414){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_44414);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e44438){if((e44438 instanceof Object)){
var ex__26709__auto__ = e44438;
var statearr_44439_44457 = state_44414;
(statearr_44439_44457[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_44414);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e44438;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__44458 = state_44414;
state_44414 = G__44458;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_44414){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_44414);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_44440 = f__26801__auto__.call(null);
(statearr_44440[(6)] = c__26800__auto___44442);

return statearr_44440;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return out;
}));

(cljs.core.async.partition.cljs$lang$maxFixedArity = 3);

/**
 * Deprecated - this function will be removed. Use transducer instead
 */
cljs.core.async.partition_by = (function cljs$core$async$partition_by(var_args){
var G__44460 = arguments.length;
switch (G__44460) {
case 2:
return cljs.core.async.partition_by.cljs$core$IFn$_invoke$arity$2((arguments[(0)]),(arguments[(1)]));

break;
case 3:
return cljs.core.async.partition_by.cljs$core$IFn$_invoke$arity$3((arguments[(0)]),(arguments[(1)]),(arguments[(2)]));

break;
default:
throw (new Error(["Invalid arity: ",cljs.core.str.cljs$core$IFn$_invoke$arity$1(arguments.length)].join('')));

}
});

(cljs.core.async.partition_by.cljs$core$IFn$_invoke$arity$2 = (function (f,ch){
return cljs.core.async.partition_by.call(null,f,ch,null);
}));

(cljs.core.async.partition_by.cljs$core$IFn$_invoke$arity$3 = (function (f,ch,buf_or_n){
var out = cljs.core.async.chan.call(null,buf_or_n);
var c__26800__auto___44530 = cljs.core.async.chan.call(null,(1));
cljs.core.async.impl.dispatch.run.call(null,(function (){
var f__26801__auto__ = (function (){var switch__26705__auto__ = (function (state_44502){
var state_val_44503 = (state_44502[(1)]);
if((state_val_44503 === (7))){
var inst_44498 = (state_44502[(2)]);
var state_44502__$1 = state_44502;
var statearr_44504_44531 = state_44502__$1;
(statearr_44504_44531[(2)] = inst_44498);

(statearr_44504_44531[(1)] = (3));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44503 === (1))){
var inst_44461 = [];
var inst_44462 = inst_44461;
var inst_44463 = new cljs.core.Keyword("cljs.core.async","nothing","cljs.core.async/nothing",-69252123);
var state_44502__$1 = (function (){var statearr_44505 = state_44502;
(statearr_44505[(7)] = inst_44463);

(statearr_44505[(8)] = inst_44462);

return statearr_44505;
})();
var statearr_44506_44532 = state_44502__$1;
(statearr_44506_44532[(2)] = null);

(statearr_44506_44532[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44503 === (4))){
var inst_44466 = (state_44502[(9)]);
var inst_44466__$1 = (state_44502[(2)]);
var inst_44467 = (inst_44466__$1 == null);
var inst_44468 = cljs.core.not.call(null,inst_44467);
var state_44502__$1 = (function (){var statearr_44507 = state_44502;
(statearr_44507[(9)] = inst_44466__$1);

return statearr_44507;
})();
if(inst_44468){
var statearr_44508_44533 = state_44502__$1;
(statearr_44508_44533[(1)] = (5));

} else {
var statearr_44509_44534 = state_44502__$1;
(statearr_44509_44534[(1)] = (6));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44503 === (15))){
var inst_44492 = (state_44502[(2)]);
var state_44502__$1 = state_44502;
var statearr_44510_44535 = state_44502__$1;
(statearr_44510_44535[(2)] = inst_44492);

(statearr_44510_44535[(1)] = (14));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44503 === (13))){
var state_44502__$1 = state_44502;
var statearr_44511_44536 = state_44502__$1;
(statearr_44511_44536[(2)] = null);

(statearr_44511_44536[(1)] = (14));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44503 === (6))){
var inst_44462 = (state_44502[(8)]);
var inst_44487 = inst_44462.length;
var inst_44488 = (inst_44487 > (0));
var state_44502__$1 = state_44502;
if(cljs.core.truth_(inst_44488)){
var statearr_44512_44537 = state_44502__$1;
(statearr_44512_44537[(1)] = (12));

} else {
var statearr_44513_44538 = state_44502__$1;
(statearr_44513_44538[(1)] = (13));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44503 === (3))){
var inst_44500 = (state_44502[(2)]);
var state_44502__$1 = state_44502;
return cljs.core.async.impl.ioc_helpers.return_chan.call(null,state_44502__$1,inst_44500);
} else {
if((state_val_44503 === (12))){
var inst_44462 = (state_44502[(8)]);
var inst_44490 = cljs.core.vec.call(null,inst_44462);
var state_44502__$1 = state_44502;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_44502__$1,(15),out,inst_44490);
} else {
if((state_val_44503 === (2))){
var state_44502__$1 = state_44502;
return cljs.core.async.impl.ioc_helpers.take_BANG_.call(null,state_44502__$1,(4),ch);
} else {
if((state_val_44503 === (11))){
var inst_44466 = (state_44502[(9)]);
var inst_44470 = (state_44502[(10)]);
var inst_44480 = (state_44502[(2)]);
var inst_44481 = [];
var inst_44482 = inst_44481.push(inst_44466);
var inst_44462 = inst_44481;
var inst_44463 = inst_44470;
var state_44502__$1 = (function (){var statearr_44514 = state_44502;
(statearr_44514[(7)] = inst_44463);

(statearr_44514[(11)] = inst_44482);

(statearr_44514[(8)] = inst_44462);

(statearr_44514[(12)] = inst_44480);

return statearr_44514;
})();
var statearr_44515_44539 = state_44502__$1;
(statearr_44515_44539[(2)] = null);

(statearr_44515_44539[(1)] = (2));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44503 === (9))){
var inst_44462 = (state_44502[(8)]);
var inst_44478 = cljs.core.vec.call(null,inst_44462);
var state_44502__$1 = state_44502;
return cljs.core.async.impl.ioc_helpers.put_BANG_.call(null,state_44502__$1,(11),out,inst_44478);
} else {
if((state_val_44503 === (5))){
var inst_44466 = (state_44502[(9)]);
var inst_44463 = (state_44502[(7)]);
var inst_44470 = (state_44502[(10)]);
var inst_44470__$1 = f.call(null,inst_44466);
var inst_44471 = cljs.core._EQ_.call(null,inst_44470__$1,inst_44463);
var inst_44472 = cljs.core.keyword_identical_QMARK_.call(null,inst_44463,new cljs.core.Keyword("cljs.core.async","nothing","cljs.core.async/nothing",-69252123));
var inst_44473 = ((inst_44471) || (inst_44472));
var state_44502__$1 = (function (){var statearr_44516 = state_44502;
(statearr_44516[(10)] = inst_44470__$1);

return statearr_44516;
})();
if(cljs.core.truth_(inst_44473)){
var statearr_44517_44540 = state_44502__$1;
(statearr_44517_44540[(1)] = (8));

} else {
var statearr_44518_44541 = state_44502__$1;
(statearr_44518_44541[(1)] = (9));

}

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44503 === (14))){
var inst_44495 = (state_44502[(2)]);
var inst_44496 = cljs.core.async.close_BANG_.call(null,out);
var state_44502__$1 = (function (){var statearr_44520 = state_44502;
(statearr_44520[(13)] = inst_44495);

return statearr_44520;
})();
var statearr_44521_44542 = state_44502__$1;
(statearr_44521_44542[(2)] = inst_44496);

(statearr_44521_44542[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44503 === (10))){
var inst_44485 = (state_44502[(2)]);
var state_44502__$1 = state_44502;
var statearr_44522_44543 = state_44502__$1;
(statearr_44522_44543[(2)] = inst_44485);

(statearr_44522_44543[(1)] = (7));


return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
if((state_val_44503 === (8))){
var inst_44466 = (state_44502[(9)]);
var inst_44462 = (state_44502[(8)]);
var inst_44470 = (state_44502[(10)]);
var inst_44475 = inst_44462.push(inst_44466);
var tmp44519 = inst_44462;
var inst_44462__$1 = tmp44519;
var inst_44463 = inst_44470;
var state_44502__$1 = (function (){var statearr_44523 = state_44502;
(statearr_44523[(7)] = inst_44463);

(statearr_44523[(8)] = inst_44462__$1);

(statearr_44523[(14)] = inst_44475);

return statearr_44523;
})();
var statearr_44524_44544 = state_44502__$1;
(statearr_44524_44544[(2)] = null);

(statearr_44524_44544[(1)] = (2));


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
});
return (function() {
var cljs$core$async$state_machine__26706__auto__ = null;
var cljs$core$async$state_machine__26706__auto____0 = (function (){
var statearr_44525 = [null,null,null,null,null,null,null,null,null,null,null,null,null,null,null];
(statearr_44525[(0)] = cljs$core$async$state_machine__26706__auto__);

(statearr_44525[(1)] = (1));

return statearr_44525;
});
var cljs$core$async$state_machine__26706__auto____1 = (function (state_44502){
while(true){
var ret_value__26707__auto__ = (function (){try{while(true){
var result__26708__auto__ = switch__26705__auto__.call(null,state_44502);
if(cljs.core.keyword_identical_QMARK_.call(null,result__26708__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
continue;
} else {
return result__26708__auto__;
}
break;
}
}catch (e44526){if((e44526 instanceof Object)){
var ex__26709__auto__ = e44526;
var statearr_44527_44545 = state_44502;
(statearr_44527_44545[(5)] = ex__26709__auto__);


cljs.core.async.impl.ioc_helpers.process_exception.call(null,state_44502);

return new cljs.core.Keyword(null,"recur","recur",-437573268);
} else {
throw e44526;

}
}})();
if(cljs.core.keyword_identical_QMARK_.call(null,ret_value__26707__auto__,new cljs.core.Keyword(null,"recur","recur",-437573268))){
var G__44546 = state_44502;
state_44502 = G__44546;
continue;
} else {
return ret_value__26707__auto__;
}
break;
}
});
cljs$core$async$state_machine__26706__auto__ = function(state_44502){
switch(arguments.length){
case 0:
return cljs$core$async$state_machine__26706__auto____0.call(this);
case 1:
return cljs$core$async$state_machine__26706__auto____1.call(this,state_44502);
}
throw(new Error('Invalid arity: ' + arguments.length));
};
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$0 = cljs$core$async$state_machine__26706__auto____0;
cljs$core$async$state_machine__26706__auto__.cljs$core$IFn$_invoke$arity$1 = cljs$core$async$state_machine__26706__auto____1;
return cljs$core$async$state_machine__26706__auto__;
})()
})();
var state__26802__auto__ = (function (){var statearr_44528 = f__26801__auto__.call(null);
(statearr_44528[(6)] = c__26800__auto___44530);

return statearr_44528;
})();
return cljs.core.async.impl.ioc_helpers.run_state_machine_wrapped.call(null,state__26802__auto__);
}));


return out;
}));

(cljs.core.async.partition_by.cljs$lang$maxFixedArity = 3);


//# sourceMappingURL=async.js.map?rel=1654487551907
