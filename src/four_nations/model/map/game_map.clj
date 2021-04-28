(ns four-nations.model.map.game-map
  (:require [four-nations.model.map.utils :as utils]))

(defrecord GameTile [terrain-type raw-value x y])
(defrecord GameMap [game-map height width])

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Helper functions
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn print-map
  [game-map]
  (doseq [row game-map]
    (doseq [cell row]
      (print (condp = (:terrain-type cell)
               :land "G"
               :water "~"
               :mountain "^"
               :coast "."
               "?")))
    (println)))

(defn average-map-value
  "Given a noise map, calculates the average value of all of the cells within that noise map."
  [noise]
  (let [all-values (flatten noise)]
    (quot (apply + all-values)
          (count all-values))))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Initial map tile creation
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn determine-cell-terrain-type
  "Determines if a cell should be land, water, or mountain based on the cell value's relation to
   the average value of the game map."
  [cell-value average-value]
  (cond
    (> cell-value (+ average-value 15)) :mountain
    (< cell-value average-value) :water
    :else :land))

(defn cell-value->initial-game-tile
  "Given the value of a particular cell, builds a game tile that includes the basic terrain type."
  [cell-value average-value x y]
  (let [terrain-type (determine-cell-terrain-type cell-value average-value)]
    (->GameTile terrain-type cell-value x y)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Building the game map and adding variety to the terrain
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(defn spread-water
  "Given a game map and spread chance, for every land tile within the game that is near water, the
   spread chance determines how likely it is that the water will spread to that tile."
  [{:keys [game-map height width]} spread-chance]
  (-> (fn [tile x y]
        (let [neighbors (utils/coordinates->neighbors height width game-map [x y])]
          (if (and (-> tile :terrain-type (= :land))
                   (->> neighbors
                        (map :terrain-type)
                        (some (partial = :water)))
                   (< (rand) spread-chance))
            (assoc tile :terrain-type :water)
            tile)))
      (utils/two-dimensional-map game-map height width)
      (->GameMap height width)))

(defn drench-surrounded-land
  "Given a game map, finds any land tiles where all of its cardinal neighbors are water and sets
   those tiles to also be water. This prevents tiny, nonsensical islands."
  [{:keys [game-map height width]}]
  (-> (fn [tile x y]
        (let [neighbors (utils/coordinates->neighbors height width game-map [x y])]
          (if (and (->> neighbors (map :terrain-type) (some (partial = :water)))
                   (-> tile :terrain-type (= :land)))
            (assoc tile :terrain-type :water)
            tile)))
      (utils/two-dimensional-map game-map height width)
      (->GameMap height width)))

(defn add-coastline
  "Given a map, changes the terrain type of any land tiles that are near water to be coastline."
  [{:keys [game-map height width]}]
  (-> (fn [tile x y]
        (let [neighbors (utils/coordinates->neighbors height width game-map [x y])]
          (if (and (->> neighbors (map :terrain-type) (some (partial = :water)))
                   (-> tile :terrain-type (= :land)))
            (assoc tile :terrain-type :coast)
            tile)))
      (utils/two-dimensional-map game-map height width)
      (->GameMap height width)))

(defn noise-map->basic-game-map
  "Given the noise map and the average value of cells across the map, builds basic game map with
   simple terrain selection."
  [{:keys [noise height width]} average-value]
  (-> (fn [cell-value x y]
        (-> cell-value
            (determine-cell-terrain-type average-value)
            (->GameTile cell-value x y)))
      (utils/two-dimensional-map noise height width)
      (->GameMap height width)))

(defn noise-map->game-map
  "Given a generated and smoothed noisemap, generates a game map with terrain added."
  [noise-map & {:keys [water-spread-chance]
                :or {water-spread-chance 0.05}}]
  (let [average-value (-> noise-map :noise average-map-value)]
    (-> noise-map
        (noise-map->basic-game-map average-value)
        (spread-water water-spread-chance)
        drench-surrounded-land
        add-coastline
        )))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;; Rich comments for experimenting :)
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
(comment
  (require '[four-nations.model.map.noise-map :as nm])
  (use 'clojure.pprint)
  (let [height 60
        width 275
        water-spread-chance 0.1
        water-border 2
        smoothing-passes 15]
    (-> (nm/generate-noisemap height width smoothing-passes water-border)
        (noise-map->game-map :water-spread-chance water-spread-chance)
        :game-map
        print-map)
    )
  )
