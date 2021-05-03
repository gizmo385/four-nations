(ns four-nations.general.utils
  (:require
    [clojure.edn :as edn]
    [clojure.java.io :as io]))

(defn load-edn-resource
  "Given a filename, reads that filename from the resources/ directory and loads its EDN contents."
  [resource-filename]
  (-> resource-filename io/resource slurp edn/read-string))

(defn n-times
  "Given a function f, an initial value, and a number n, performs n calls of f. The initial value is"
  [f init n]
  (reduce
    (fn [value _idx]
      (f value))
    init
    (range n)))

(defn uuid
  "Generates version 4 UUID string."
  []
  (str (java.util.UUID/randomUUID)))

(defn map-by
  "Given a key k and collection of maps, returns a map that maps the value of k in each item in the
   collection to that item."
  [k coll]
  (zipmap (map k coll) coll))

(defn map-vals
  "Given a function f and a map m, returns a new map where f has been applied to each value in m."
  [f m]
  (into {} (for [[k v] m] [k (f v)])))
