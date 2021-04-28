(ns four-nations.model.map.utils
  (:require [clojure.core.matrix :as mat]))

(defn get-cell
  "Given a 2 dimensional game map and an (x, y) coordinate pair, retrieves the value at the
   specified cell."
  [m x y]
  (mat/mget m y x))

(defn valid-coordinate?
  "A coordinate is valid if both components are non-negative integers and they are both less than
   their arespective bounds (y < height, x < width). Assumes zero-indexed coordinates."
  [height width [x y]]
  (and (nat-int? x) (nat-int? y) (< y height) (< x width)))

(defn coordinates->neighbor-coordinates
  "Given a height, a width, and a (x, y) coordinate pair, returns all valid coordinates that are
   neighbors. Valid coordinates are checked against the valid-coordinate? predicate"
  [height width [x y]]
  (let [raw-neighbors [[x (inc y)]
                       [x (dec y)]
                       [(inc x) y]
                       [(dec x) y]
                       [(inc x) (inc y)]
                       [(inc x) (dec y)]
                       [(dec x) (inc y)]
                       [(dec x) (dec y)]]]
    raw-neighbors
    (filter (partial valid-coordinate? height width) raw-neighbors)))

(defn coordinates->neighbors
  "Given a height, a width, a map m, and a (x, y) coordinate pair, returns the values of all valid
   neighbors, as defined by coordinates->neighbor-coordinates."
  [height width m [x y]]
  (->> (coordinates->neighbor-coordinates height width [x y])
       (map (fn [[x y]] (get-cell m x y)))))
