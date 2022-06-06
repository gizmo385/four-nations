(ns four-nations.general.utils
  (:require [clojure.edn :as edn]))

#_(def ^:private fs (js/require "fs"))

(defn read-file
  "Read the file at the specified path"
  [path]
  (.readFile fs path "utf8"))

#_(defn load-edn-resource
  "Read the EDN file at the specified path"
  [path]
  (edn/read-string (read-file path)))

(defn n-times
  "Given a function f, an initial value, and a number n, performs n calls of f. The initial value is"
  [f init n]
  (reduce
    (fn [value _idx]
      (f value))
    init
    (range n)))

(defn map-by
  "Given a key k and collection of maps, returns a map that maps the value of k in each item in the
   collection to that item."
  [k coll]
  (zipmap (map k coll) coll))

(defn map-vals
  "Given a function f and a map m, returns a new map where f has been applied to each value in m."
  [f m]
  (into {} (for [[k v] m] [k (f v)])))
