(ns four-nations.model.map.game-map
  (:require [four-nations.model.map.utils :as utils]))

(defrecord GameTile [terrain-type raw-value x y])
(defrecord GameMap [game-map height width])

(defn print-map
  [game-map]
  (doseq [row game-map]
    (doseq [cell row]
      (print (condp = (:terrain-type cell)
               :plains "G"
               :water "~"
               :mountain "^"
               :coast "."
               "?")))
    (println)))

(defn average-map-value
  [noise]
  (let [all-values (flatten noise)]
    (quot (apply + all-values)
          (count all-values))))

(defn determine-cell-terrain-type
  [cell-value average-value]
  (cond
    (> cell-value (+ average-value 20)) :mountain
    (< cell-value average-value) :water
    :else :plains))

(defn cell-value->initial-game-tile
  "Given the value of a particular cell, builds a game tile that includes the basic terrain type."
  [cell-value average-value x y]
  (let [terrain-type (determine-cell-terrain-type cell-value average-value)]
    (->GameTile terrain-type cell-value x y)))

(defn spread-water
  [{:keys [game-map height width]} average-value]
  (-> (fn [tile x y]
        (let [neighbors (utils/coordinates->neighbors height width game-map [x y] false)]
          tile))
      (utils/two-dimensional-map game-map height width)
      (->GameMap height width)))

(defn noise-map->basic-game-map
  [{:keys [noise height width]} average-value]
  (-> (fn [cell-value x y]
        (-> cell-value
            (determine-cell-terrain-type average-value)
            (->GameTile cell-value x y)))
      (utils/two-dimensional-map noise height width)
      (->GameMap height width)))

(defn noise-map->game-map
  [noise-map]
  (let [average-value (-> noise-map :noise average-map-value)]
    (noise-map->basic-game-map noise-map average-value)))

(comment
  (require '[four-nations.model.map.noise-map :as nm])
  (use 'clojure.pprint)
  (let [height 50
        width 250
        smoothing-passes 20]
    (->> (nm/generate-noisemap height width smoothing-passes)
         noise-map->game-map
         :game-map
         print-map)
    )
  )
