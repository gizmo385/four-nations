(ns four-nations.model.map.noise-map
  (:require
    [four-nations.general.types :refer [->Dimension ->Point ->MapTile]]
    [four-nations.general.utils :as utils]
    [four-nations.model.map.utils :as map-utils]
    [random-seed.core :as rs]))

(defn generate-points
  "Given a specified dimension, generates a mapping from coordinates to map tiles with that
   dimension."
  ([dim]
   (generate-points dim (constantly 0)))

  ([dim value-fn]
   (->> (for [x (range (:width dim))
              y (range (:height dim))
              :let [point (->Point x y)
                    tile (->MapTile point (value-fn) {})]]
          [point tile])
        (into {}))))

(defn generate-2d-noise
  "Given a height and a width, generates a two-dimensional array of noise, where each cell is
   occupied by a number between 1 and 255 (inclusive on both sides)."
  [dim]
  (letfn [(cell-value [] (inc (rs/rand-int 255)))]
    (generate-points dim cell-value)))

(defn average-of-neighbors
  "Given a 2-dimensional array of noise, a height, width, and a cell coordinate, calculates the
   average of the values of its neighbor cells"
  [noise dimension point]
  (let [neighbors (map-utils/point->neighbors dimension noise point)
        neighbor-values (map :value neighbors)]
    (quot (apply + neighbor-values)
          (count neighbor-values))))

(defn average-2d-noise
  "Given a 2-dimensional array of noise, returns a 2-dimensional array of averaged noise, where the
   value of each cell is the average of the values of its neighbors."
  [noise dimension]
  (->> noise
       (map (fn [[point tile]]
              [point (assoc tile :value (average-of-neighbors noise dimension point))]))
       (into {})))

(defn smooth-noisemap
  [noise dimension smoothing-passes]
  (->> (utils/n-times
         #(average-2d-noise % dimension)
         noise
         smoothing-passes)))

(defn generate-noisemap
  "Given a dimension, generates a 2-dimensional map of averaged noise, which can be used for
   procedural terrain generation. You can optionally supply the number of times that the noise will
   be smoothed out (defaults to only a single smoothing pass)."
  ([dimension]
   (generate-noisemap dimension 1))

  ([dimension smoothing-passes]
   (-> (generate-2d-noise dimension)
       (smooth-noisemap dimension smoothing-passes))))
