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
