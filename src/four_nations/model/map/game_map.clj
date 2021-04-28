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
    (> cell-value (+ average-value 40)) :mountain
    (< cell-value average-value) :water
    :else :plains))

(defn cell-value->cell
  [cell-value average-value x y]
  (let [terrain-type (determine-cell-terrain-type cell-value average-value)]
    (->GameTile terrain-type cell-value x y)))

(defn noise-map->basic-game-map
  [{:keys [noise height width]} average-value]
  (->GameMap
    (for [y (range height)]
      (for [x (range width)]
        (let [cell-value (utils/get-cell noise x y)]
          (cell-value->cell cell-value average-value x y))))
    height
    width))

(defn noise-map->game-map
  [noise-map]
  (let [average-value (-> noise-map :noise average-map-value)]
    (noise-map->basic-game-map noise-map average-value)))

(comment
  (require '[four-nations.model.map.noise-map :as nm])
  (use 'clojure.pprint)
  (print-map (:game-map (noise-map->game-map (nm/generate-noisemap 50 250)))))
